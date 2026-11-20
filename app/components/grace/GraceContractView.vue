<script setup lang="ts">
import type { GraceContract } from '~/types/grace'

defineProps<{
  contract: GraceContract
}>()

const sectionHeading = 'text-[10px] font-medium text-muted-foreground uppercase tracking-wide mb-1'
</script>

<template>
  <div class="space-y-4">
    <section>
      <h4 :class="sectionHeading">Problem</h4>
      <p v-if="contract.problem" class="text-xs whitespace-pre-line">{{ contract.problem }}</p>
      <p v-else class="text-xs text-muted-foreground">—</p>
    </section>

    <section>
      <h4 :class="sectionHeading">Change</h4>
      <p v-if="contract.change" class="text-xs whitespace-pre-line">{{ contract.change }}</p>
      <p v-else class="text-xs text-muted-foreground">—</p>
    </section>

    <section>
      <h4 :class="sectionHeading">Constraints</h4>
      <p v-if="contract.constraints.length === 0" class="text-xs text-muted-foreground">None</p>
      <ul v-else class="space-y-1.5">
        <li v-for="constraint in contract.constraints" :key="constraint.id" class="text-xs">
          <div class="flex items-start gap-1.5">
            <span class="font-mono text-[10px] text-muted-foreground shrink-0 mt-0.5">{{ constraint.id }}</span>
            <span class="whitespace-pre-line">{{ constraint.text }}</span>
          </div>
          <div
            v-if="constraint.rule"
            class="mt-0.5 ml-3 text-[10px] text-muted-foreground border-l border-border pl-2"
          >
            Rule: <span class="font-mono">{{ constraint.rule }}</span>
          </div>
        </li>
      </ul>
    </section>

    <section>
      <h4 :class="sectionHeading">Non-Goals</h4>
      <p v-if="contract.nonGoals.length === 0" class="text-xs text-muted-foreground">None</p>
      <ul v-else class="space-y-1">
        <li v-for="goal in contract.nonGoals" :key="goal.id" class="text-xs flex items-start gap-1.5">
          <span class="font-mono text-[10px] text-muted-foreground shrink-0 mt-0.5">{{ goal.id }}</span>
          <span class="whitespace-pre-line">{{ goal.text }}</span>
        </li>
      </ul>
    </section>

    <section>
      <h4 :class="sectionHeading">Acceptance</h4>
      <p v-if="contract.acceptance.length === 0" class="text-xs text-muted-foreground">None</p>
      <ul v-else class="space-y-1.5">
        <li v-for="criterion in contract.acceptance" :key="criterion.id" class="text-xs">
          <div class="flex items-start gap-1.5">
            <span class="font-mono text-[10px] text-muted-foreground shrink-0 mt-0.5">{{ criterion.id }}</span>
            <span class="whitespace-pre-line">{{ criterion.text }}</span>
          </div>
          <div v-if="criterion.checks.length > 0" class="mt-1 ml-3 flex flex-wrap items-center gap-1">
            <span class="text-[10px] text-muted-foreground">Checks:</span>
            <span
              v-for="checkId in criterion.checks"
              :key="checkId"
              class="inline-flex px-1 py-0.5 rounded bg-muted font-mono text-[10px]"
            >
              {{ checkId }}
            </span>
          </div>
        </li>
      </ul>
    </section>

    <section>
      <h4 :class="sectionHeading">Affected</h4>
      <div class="grid gap-3 sm:grid-cols-3">
        <div>
          <div class="text-[10px] text-muted-foreground mb-0.5">Paths</div>
          <ul v-if="contract.affected.paths.length > 0" class="space-y-0.5">
            <li v-for="path in contract.affected.paths" :key="path" class="text-xs font-mono break-all">{{ path }}</li>
          </ul>
          <p v-else class="text-xs text-muted-foreground">—</p>
        </div>
        <div>
          <div class="text-[10px] text-muted-foreground mb-0.5">Names</div>
          <ul v-if="contract.affected.names.length > 0" class="space-y-0.5">
            <li v-for="name in contract.affected.names" :key="name" class="text-xs break-all">{{ name }}</li>
          </ul>
          <p v-else class="text-xs text-muted-foreground">—</p>
        </div>
        <div>
          <div class="text-[10px] text-muted-foreground mb-0.5">Protected</div>
          <ul v-if="contract.affected.protected.length > 0" class="space-y-0.5">
            <li v-for="path in contract.affected.protected" :key="path" class="text-xs font-mono break-all">{{ path }}</li>
          </ul>
          <p v-else class="text-xs text-muted-foreground">—</p>
        </div>
      </div>
    </section>

    <section v-if="contract.environments.length > 0">
      <h4 :class="sectionHeading">Environments</h4>
      <ul class="space-y-1">
        <li v-for="environment in contract.environments" :key="environment.id" class="text-xs">
          <span class="font-mono text-[10px] text-muted-foreground">{{ environment.id }}</span>
          <span v-if="environment.device" class="ml-1 text-muted-foreground">device: {{ environment.device }}</span>
          <span v-if="environment.build" class="ml-1 text-muted-foreground">build: {{ environment.build }}</span>
          <div class="whitespace-pre-line">{{ environment.text }}</div>
        </li>
      </ul>
    </section>

    <section>
      <h4 :class="sectionHeading">Decisions</h4>
      <p v-if="contract.decisions.length === 0" class="text-xs text-muted-foreground">None</p>
      <ul v-else class="space-y-1">
        <li v-for="decision in contract.decisions" :key="decision.id" class="text-xs flex items-start gap-1.5">
          <span class="font-mono text-[10px] text-muted-foreground shrink-0 mt-0.5">{{ decision.id }}</span>
          <span class="whitespace-pre-line">{{ decision.text }}</span>
        </li>
      </ul>
    </section>
  </div>
</template>
