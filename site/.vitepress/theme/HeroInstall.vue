<script setup lang="ts">
import { ref } from 'vue'
import { useData } from 'vitepress'

const { frontmatter } = useData()
const copied = ref(false)

async function copy() {
  try {
    await navigator.clipboard.writeText(frontmatter.value.install.command)
    copied.value = true
    setTimeout(() => (copied.value = false), 1600)
  } catch {
    // Clipboard access can be blocked; the command stays selectable.
  }
}
</script>

<template>
  <div v-if="frontmatter.install" class="install">
    <span class="install-label">{{ frontmatter.install.label }}</span>
    <code class="install-command"><span class="prompt">$</span> {{ frontmatter.install.command }}</code>
    <button class="install-copy" type="button" :aria-label="frontmatter.install.copy" @click="copy">
      {{ copied ? frontmatter.install.copied : frontmatter.install.copy }}
    </button>
  </div>
</template>

<style scoped>
.install {
  display: flex;
  align-items: center;
  gap: 12px;
  box-sizing: border-box;
  width: 100%;
  max-width: 420px;
  margin-top: 28px;
  padding: 8px 8px 8px 14px;
  border: 1px solid var(--vp-c-divider);
  border-radius: 10px;
  background: var(--vp-c-bg-elv);
}

.install-label {
  flex: none;
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--vp-c-text-3);
}

.install-command {
  flex: 1;
  min-width: 0;
  overflow-x: auto;
  white-space: nowrap;
  font-family: var(--vp-font-family-mono);
  font-size: 14px;
  color: var(--vp-c-text-1);
  background: none;
}

.prompt {
  color: var(--vp-c-brand-1);
  user-select: none;
}

.install-copy {
  flex: none;
  padding: 4px 12px;
  border: 1px solid var(--vp-c-divider);
  border-radius: 6px;
  font-size: 13px;
  font-weight: 500;
  color: var(--vp-c-text-2);
  transition: border-color 0.2s, color 0.2s;
}

.install-copy:hover {
  border-color: var(--vp-c-brand-1);
  color: var(--vp-c-brand-1);
}

@media (min-width: 640px) {
  .install {
    margin-top: 32px;
  }
}

@media (max-width: 960px) {
  .install {
    margin-left: auto;
    margin-right: auto;
  }
}
</style>
