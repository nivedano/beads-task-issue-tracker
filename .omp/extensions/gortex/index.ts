// Gortex read-discipline bridge for omp (Oh My Pi).
//
// Port of pi-gortex v0.5.0 (https://github.com/gortexhq/pi, Apache-2.0) to
// omp's extension API. omp has no pi.registerMcpServer and names MCP tools
// mcp__gortex_<tool> (single underscore), so the MCP channel stays owned by
// .omp/mcp.json; this extension wires the other three channels pi-gortex
// provides, over the same `gortex hook --agent=pi` wire protocol:
//   - enforcement: non-graph tool calls are checked; deny postures block
//   - session orientation: injected once per session as a hidden message
//   - prompt hints: graph symbols relevant to the prompt ride the turn
// Not ported: MCP registration (omp config owns it), edit overlays and tool
// renderers (editor-side extras).
//
// Config, first hit wins: env GORTEX_BIN / GORTEX_HOOK_MODE / GORTEX_ENFORCE,
// then <cwd>/.omp/gortex.json ({ "hook_mode", "enforce" }; bin is env-only so
// a checked-out repo cannot pick the executable). Defaults: gortex on PATH,
// deny, enforce=true. Fail-open everywhere: a missing binary, down daemon,
// timeout, or parse error costs the session its guidance, never a turn.

import { execFile } from "node:child_process";
import { accessSync, constants, readFileSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { delimiter, extname, join } from "node:path";
import type { ExtensionAPI } from "@oh-my-pi/pi-coding-agent";

/** Envelope `gortex hook --agent=pi` reads on stdin (hooks.BridgeEvent). */
interface PiEnvelope {
	event: string;
	tool_name?: string;
	tool_input?: Record<string, unknown>;
	cwd?: string;
	session_id?: string;
	prompt?: string;
	is_gortex_tool?: boolean;
}

/** Decision the Go side writes back (hooks.BridgeDecision). */
interface PiDecision {
	block?: boolean;
	reason?: string;
	additional_context?: string;
	orientation?: string;
}

/** Runtime surface this bridge reads off the handler context. omp's public
 * ExtensionContext type does not expose all of it (buildContextEntries,
 * sessionId, signal), so each handler casts once at this boundary. */
interface BridgeContext {
	cwd?: string;
	signal?: AbortSignal;
	sessionManager?: {
		getSessionId?: () => string;
		sessionId?: string;
		buildContextEntries?: () => Array<Record<string, unknown>>;
	};
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

interface GortexConfig {
	hookArgv: string[];
	enforce: boolean;
}

function normalizeMode(mode: unknown): string {
	switch (String(mode ?? "").trim().toLowerCase()) {
		case "enrich":
			return "enrich";
		case "consult-unlock":
			return "consult-unlock";
		case "nudge":
		case "adaptive-nudge":
			return "nudge";
		default:
			return "deny";
	}
}

function parseBoolean(value: unknown): boolean | undefined {
	switch (String(value ?? "").trim().toLowerCase()) {
		case "1":
		case "true":
		case "yes":
		case "on":
			return true;
		case "0":
		case "false":
		case "no":
		case "off":
			return false;
		default:
			return undefined;
	}
}

function resolveConfig(cwd: string): GortexConfig {
	let sidecar: Record<string, unknown> = {};
	try {
		const parsed: unknown = JSON.parse(readFileSync(join(cwd, ".omp", "gortex.json"), "utf8"));
		if (parsed !== null && typeof parsed === "object") sidecar = parsed as Record<string, unknown>;
	} catch {
		// absent or malformed: fall through to defaults
	}
	const env = process.env;
	const bin = env.GORTEX_BIN?.trim() || "gortex";
	const mode = normalizeMode(env.GORTEX_HOOK_MODE ?? sidecar.hook_mode);
	const enforce =
		parseBoolean(env.GORTEX_ENFORCE) ??
		(typeof sidecar.enforce === "boolean" ? sidecar.enforce : true);
	const hookArgv = [bin, "hook", "--agent=pi", "--no-progress"];
	// "deny" is the hook's own default, so it rides as an absent flag.
	if (mode !== "deny") hookArgv.push(`--mode=${mode}`);
	return { hookArgv, enforce };
}

// ---------------------------------------------------------------------------
// Process bridge
// ---------------------------------------------------------------------------

/** The file `bin` names, directly or through PATH (PATHEXT-aware on Windows). */
function resolveExecutable(bin: string): string | undefined {
	const expanded = bin === "~" ? homedir() : bin.startsWith("~/") || bin.startsWith("~\\") ? join(homedir(), bin.slice(2)) : bin;
	const windows = process.platform === "win32";
	const exts = windows ? (process.env.PATHEXT ?? ".EXE;.CMD;.BAT;.COM").split(";").filter(Boolean) : [""];
	if (windows && extname(expanded) !== "") exts.unshift("");
	const candidates = /[\\/]/.test(expanded)
		? [expanded]
		: (process.env.PATH ?? "").split(delimiter).filter(Boolean).map((dir) => join(dir, expanded));
	for (const path of candidates) {
		for (const ext of exts) {
			try {
				accessSync(path + ext, windows ? constants.F_OK : constants.X_OK);
				if (statSync(path + ext).isFile()) return path + ext;
			} catch {
				// not this one
			}
		}
	}
	return undefined;
}

/**
 * Sends one envelope to the hook bridge and parses its decision. Fail-open:
 * any error, abort, timeout, or non-object reply returns an empty decision.
 */
function callHook(argv: string[], envelope: PiEnvelope, signal?: AbortSignal): Promise<PiDecision> {
	const { promise, resolve } = Promise.withResolvers<PiDecision>();
	const [bin, ...args] = argv;
	if (!bin) {
		resolve({});
		return promise;
	}
	const settle = (stdout: string | undefined): void => {
		try {
			const out = (stdout ?? "").trim();
			if (!out) return resolve({});
			const decision: unknown = JSON.parse(out);
			if (typeof decision !== "object" || decision === null || Array.isArray(decision)) return resolve({});
			const d = decision as PiDecision;
			// Go guidance spells pi-style mcp__gortex__<tool>; omp mounts
			// mcp__gortex_<tool>. Rewrite so injected names are callable.
			for (const key of ["reason", "additional_context", "orientation"] as const) {
				const text = d[key];
				if (typeof text === "string") d[key] = text.replaceAll("mcp__gortex__", "mcp__gortex_");
			}
			resolve(d);
		} catch {
			resolve({});
		}
	};
	let child;
	try {
		const file = process.platform === "win32" ? resolveExecutable(bin) : bin;
		if (!file) {
			resolve({});
			return promise;
		}
		const opts = { windowsHide: true, timeout: 5_000, maxBuffer: 16 * 1024 * 1024, encoding: "utf8" as const, signal };
		if (/\.(cmd|bat)$/i.test(file)) {
			// Node cannot spawn batch files directly; route through cmd.exe.
			// Args here are fixed flags; only the binary path needs quoting.
			const line = [`"${file}"`, ...args].join(" ");
			child = execFile(process.env.ComSpec || process.env.COMSPEC || "cmd.exe", ["/d", "/s", "/c", line], { ...opts, windowsVerbatimArguments: true }, (_err, stdout) => settle(stdout));
		} else {
			child = execFile(file, args, opts, (_err, stdout) => settle(stdout));
		}
	} catch {
		resolve({});
		return promise;
	}
	child.stdin?.on("error", () => {});
	child.stdin?.end(JSON.stringify(envelope));
	return promise;
}

// ---------------------------------------------------------------------------
// Tool-call normalization (omp vocabulary -> canonical Claude-Code vocabulary)
// ---------------------------------------------------------------------------
//
// The Go classifier switches on Claude-Code tool names and canonical input
// keys ("file_path"/"pattern"/"command"). pi-gortex's map, adjusted for omp:
// `glob` is omp's file-glob surface (pi spells it find/ls); `ast_grep` and
// `ast_edit` are source search/mutation and belong in the Grep/Edit classes.
// Deliberately unmapped: `find` (semantic behavior search, not a raw source
// read — a Glob redirect would misdescribe it), `eval` (arbitrary code, not
// classifiable by tool name), `lsp` (already code intelligence). Unknown
// names pass through to the classifier untouched.

const toolNameMap: Record<string, string> = {
	read: "Read",
	grep: "Grep",
	glob: "Glob",
	ast_grep: "Grep",
	bash: "Bash",
	edit: "Edit",
	ast_edit: "Edit",
	write: "Write",
};

function firstString(obj: Record<string, unknown>, keys: string[]): string | undefined {
	for (const k of keys) {
		const v = obj[k];
		if (typeof v === "string" && v !== "") return v;
	}
	return undefined;
}

function normalizeToolCall(name: string, input: Record<string, unknown>): { tool_name: string; tool_input: Record<string, unknown> } {
	const canonical = toolNameMap[name.toLowerCase()] ?? name;
	const out: Record<string, unknown> = { ...input };
	const path = firstString(input, ["file_path", "path", "file", "filepath", "absolute_path"]);
	if (path !== undefined) out.file_path = path;
	let pattern = firstString(input, ["pattern", "glob", "query", "regex", "name"]);
	if (pattern === undefined && canonical === "Glob") pattern = path;
	if (pattern !== undefined) out.pattern = pattern;
	const command = firstString(input, ["command", "cmd", "script"]);
	if (command !== undefined) out.command = command;
	return { tool_name: canonical, tool_input: out };
}

// ---------------------------------------------------------------------------
// Extension factory
// ---------------------------------------------------------------------------

/** omp mounts MCP tools as mcp__<server>_<tool>; pi uses mcp__<server>__<tool>. */
const GORTEX_TOOL_RE = /^mcp__gortex(?:__|_)/;
const isGortexTool = (name: string): boolean => GORTEX_TOOL_RE.test(name);

const GORTEX_MESSAGE_TYPE = "gortex";

export default function gortexExtension(pi: ExtensionAPI): void {
	let config = resolveConfig(process.cwd());
	let briefing = "";
	let briefingCompacted = false;
	let startupHookRan = false;

	// Resolves once this instance's session_start has run; a fast first prompt
	// must not outrun config resolution.
	const { promise: sessionStarted, resolve: markStarted } = Promise.withResolvers<void>();

	function waitSession(ms: number): Promise<void> {
		const { promise, resolve } = Promise.withResolvers<void>();
		const timer = setTimeout(resolve, ms);
		if (typeof timer !== "number") timer.unref?.();
		void sessionStarted.then(() => {
			clearTimeout(timer);
			resolve();
		});
		return promise;
	}

	function toolsReady(): boolean {
		try {
			return pi.getAllTools().some((tool) => isGortexTool(tool.name));
		} catch {
			return false;
		}
	}

	async function waitTools(ms: number): Promise<boolean> {
		const deadline = Date.now() + ms;
		while (!toolsReady()) {
			if (Date.now() >= deadline) return false;
			const { promise, resolve } = Promise.withResolvers<void>();
			setTimeout(resolve, 100);
			await promise;
		}
		return true;
	}

	/** Whether a briefing already sits in what the model sees (resume/reload). */
	function briefingInContext(bridge: BridgeContext | undefined): boolean {
		try {
			const entries = bridge?.sessionManager?.buildContextEntries?.() ?? [];
			return entries.some((entry) => {
				if (entry.type !== "custom_message" || entry.customType !== GORTEX_MESSAGE_TYPE) return false;
				const details = entry.details;
				return typeof details === "object" && details !== null && "briefing" in details && details.briefing === true;
			});
		} catch {
			return false;
		}
	}

	const sessionIdOf = (bridge: BridgeContext | undefined): string =>
		bridge?.sessionManager?.getSessionId?.() ?? bridge?.sessionManager?.sessionId ?? "";

	pi.on("session_start", (_event, ctx) => {
		config = resolveConfig(ctx?.cwd ?? process.cwd());
		markStarted();
	});

	pi.on("session_compact", () => {
		briefingCompacted = true;
	});

	// Fires before the agent loop's first LLM call of a prompt. The returned
	// hidden message is stored after the prompt, so later calls carry it and
	// the prefix-cached system prompt stays untouched. omp delivers the prompt
	// text on this event, so no separate `input` wiring is needed.
	pi.on("before_agent_start", async (event, ctx): Promise<{ message: { customType: string; content: string; display: boolean; details: { briefing: boolean } } } | void> => {
		const bridge = ctx as BridgeContext | undefined; // runtime-optional members, see BridgeContext
		const cwd = bridge?.cwd ?? process.cwd();
		const session_id = sessionIdOf(bridge);
		const parts: string[] = [];
		let briefed = false;
		if (!startupHookRan) {
			startupHookRan = true; // before the first await, so a retry cannot race
			await waitSession(15_000);
			const ready = await waitTools(10_000);
			const decision = await callHook(config.hookArgv, { event: "session_start", cwd, session_id });
			if (!ready) {
				parts.push(
					"[Gortex] graph tools are unavailable this session (the gortex MCP server did not register tools). " +
						"Use omp's own tools; if the user wants graph tools, tell them to check .omp/mcp.json and the gortex daemon.",
				);
			}
			if (decision.orientation) {
				briefing = decision.orientation;
				briefed = !briefingInContext(bridge);
				if (briefed) parts.push(decision.orientation);
			}
		} else if (briefingCompacted) {
			briefingCompacted = false;
			if (briefing !== "" && !briefingInContext(bridge)) {
				parts.push(briefing);
				briefed = true;
			}
		}
		const prompt = "prompt" in event ? event.prompt : undefined;
		if (typeof prompt === "string" && prompt.trim() !== "" && toolsReady()) {
			const decision = await callHook(config.hookArgv, { event: "prompt", prompt, cwd, session_id });
			if (decision.additional_context) parts.push(decision.additional_context);
		}
		if (parts.length === 0) return;
		return { message: { customType: GORTEX_MESSAGE_TYPE, content: parts.join("\n\n"), display: false, details: { briefing: briefed } } };
	});

	// Enforcement: every non-graph tool call is checked against the Go hook.
	// Without graph tools, discipline would block omp's tools with nothing to
	// use instead, so it stays off until the gortex MCP tools are present.
	pi.on("tool_call", async (event, ctx) => {
		if (!config.enforce || !toolsReady()) return;
		const bridge = ctx as BridgeContext | undefined; // runtime-optional members, see BridgeContext
		const name = event.toolName ?? "";
		const input = (event.input ?? {}) as Record<string, unknown>; // tool input is an open key/value bag
		const gortexCall = isGortexTool(name);
		// A graph tool keeps its own name: Gortex's `read` is no omp `read`.
		const norm = gortexCall
			? { tool_name: name.replace(GORTEX_TOOL_RE, ""), tool_input: input }
			: normalizeToolCall(name, input);
		const decision = await callHook(
			config.hookArgv,
			{
				event: "tool_call",
				tool_name: norm.tool_name,
				tool_input: norm.tool_input,
				cwd: bridge?.cwd ?? process.cwd(),
				session_id: sessionIdOf(bridge),
				is_gortex_tool: gortexCall,
			},
			bridge?.signal,
		);
		if (decision.block) {
			return { block: true, reason: decision.reason ?? "[Gortex] blocked: prefer graph tools." };
		}
		// omp delivers additionalContext after the tool result, before the next
		// provider request — no guidance parking needed.
		if (decision.additional_context) {
			return { additionalContext: decision.additional_context };
		}
	});
}
