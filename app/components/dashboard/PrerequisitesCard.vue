<script setup lang="ts">
import LinkifiedText from '~/components/ui/linkified-text/LinkifiedText.vue'

const prerequisitesContent = `**Beads** is an AI-native issue tracker that lives directly in your codebase. Issues are stored as files in a \`.beads\` folder within your repository.

This app works with [\`br\`](https://github.com/Dicklesworthstone/beads_rust) (beads_rust), the SQLite + JSONL Beads CLI.

### Prerequisites

#### 1. Install the Beads Rust CLI

Follow the install instructions at [github.com/Dicklesworthstone/beads_rust](https://github.com/Dicklesworthstone/beads_rust), then verify:

\`\`\`bash
br --version
\`\`\`

#### 2. Initialize Beads in your project

Navigate to your project folder and run:

\`\`\`bash
cd /path/to/your/project
br init
\`\`\`

This creates a \`.beads\` folder in your repository.

#### 3. Create your first issue

\`\`\`bash
br create "My first issue" --type task --priority p2
\`\`\`

### Learn more

- [Beads Rust Documentation](https://github.com/Dicklesworthstone/beads_rust)
- [Beads Rust Releases](https://github.com/Dicklesworthstone/beads_rust/releases)
`
</script>

<template>
  <div class="flex flex-col items-center justify-center h-full p-8 my-4">
    <div class="max-w-2xl w-full bg-card border border-border rounded-lg p-6 space-y-6">
      <!-- Header with icon -->
      <div class="flex items-center gap-3 pb-4 border-b border-border">
        <div class="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center">
          <svg class="w-5 h-5 text-primary" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="10" />
            <path d="M12 16v-4" />
            <path d="M12 8h.01" />
          </svg>
        </div>
        <div>
          <h2 class="text-lg font-semibold">Welcome to Beads Task-Issue Tracker</h2>
          <p class="text-sm text-muted-foreground">Follow the steps below to get started</p>
        </div>
      </div>

      <!-- Markdown content -->
      <div class="markdown-base compact">
        <LinkifiedText :text="prerequisitesContent" />
      </div>

    </div>
  </div>
</template>
