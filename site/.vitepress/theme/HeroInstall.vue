<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useData, withBase } from 'vitepress'

type Platform = 'windows' | 'unix' | 'python'

const { frontmatter } = useData()
const selected = ref<Platform>('windows')
const copied = ref(false)

const choices = computed(() => [
  { key: 'windows' as const, prompt: '>', label: frontmatter.value.install?.windowsLabel || 'Windows', command: frontmatter.value.install?.windows },
  { key: 'unix' as const, prompt: '$', label: frontmatter.value.install?.unixLabel || 'Linux / macOS / Termux', command: frontmatter.value.install?.unix },
  { key: 'python' as const, prompt: '$', label: frontmatter.value.install?.pythonLabel || 'Python (uv)', command: frontmatter.value.install?.python },
])
const active = computed(() => choices.value.find(c => c.key === selected.value) || choices.value[0])
const moreLink = computed(() => withBase(frontmatter.value.install?.moreLink ?? '/installation'))

// The page is prerendered with Windows selected; switch to the visitor's platform after hydration.
onMounted(() => {
  if (!/Windows/i.test(navigator.userAgent)) selected.value = 'unix'
})

function select(key: Platform) {
  selected.value = key
  copied.value = false
}

async function copy() {
  if (!active.value?.command) return
  try {
    await navigator.clipboard.writeText(active.value.command)
    copied.value = true
    setTimeout(() => (copied.value = false), 1600)
  } catch {
    // Clipboard permissions vary by browser; the command remains selectable.
  }
}
</script>

<template>
  <div v-if="frontmatter.install" class="installer">
    <div class="installer-heading">{{ frontmatter.install.label }}</div>
    <div class="installer-tabs" role="group" :aria-label="frontmatter.install.label">
      <button
        v-for="choice in choices"
        :key="choice.key"
        type="button"
        class="installer-tab"
        :class="{ active: selected === choice.key }"
        :aria-pressed="selected === choice.key"
        @click="select(choice.key)"
      >{{ choice.label }}</button>
    </div>
    <div class="installer-command">
      <code><span class="prompt">{{ active.prompt }}</span> {{ active.command }}</code>
      <button type="button" class="installer-copy" @click="copy">
        {{ copied ? frontmatter.install.copied : frontmatter.install.copy }}
      </button>
      <span class="visually-hidden" aria-live="polite">{{ copied ? frontmatter.install.copied : '' }}</span>
    </div>
    <a class="installer-more" :href="moreLink">{{ frontmatter.install.moreLabel }} →</a>
  </div>
</template>

<style scoped>
.installer { width: 100%; max-width: 580px; margin-top: 26px; }
.installer-heading { margin-bottom: 10px; font-size: 13px; font-weight: 600; color: var(--vp-c-text-2); }
.installer-tabs { display: flex; flex-wrap: wrap; gap: 6px; margin-bottom: 8px; }
.installer-tab {
  padding: 5px 10px; border: 1px solid var(--vp-c-divider); border-radius: 7px;
  font-size: 12px; line-height: 1.6; color: var(--vp-c-text-2); background: var(--vp-c-bg-elv);
}
.installer-tab:hover { border-color: var(--vp-c-brand-2); }
.installer-tab.active { color: var(--vp-c-brand-1); border-color: var(--vp-c-brand-1); background: var(--vp-c-brand-soft); }
.installer-command {
  display: flex; align-items: center; gap: 10px; padding: 12px;
  border: 1px solid var(--vp-c-divider); border-radius: 9px;
  background: var(--vp-code-block-bg);
}
.installer-command code {
  flex: 1; min-width: 0; overflow-x: auto; white-space: nowrap;
  font-size: 12px; line-height: 1.6; color: var(--vp-c-text-1);
}
.prompt { color: var(--vp-c-brand-1); user-select: none; }
.installer-copy {
  flex: none; padding: 5px 10px; border: 1px solid var(--vp-c-divider);
  border-radius: 6px; font-size: 12px; font-weight: 600; color: var(--vp-c-text-2);
}
.installer-copy:hover { color: var(--vp-c-brand-1); border-color: var(--vp-c-brand-1); }
.installer-more { display: inline-block; margin-top: 9px; font-size: 12px; color: var(--vp-c-brand-1); }
@media (max-width: 959px) { .installer { margin-left: auto; margin-right: auto; } }
</style>
