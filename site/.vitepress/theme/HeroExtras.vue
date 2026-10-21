<script setup lang="ts">
import { ref } from 'vue'
import { useData, withBase } from 'vitepress'

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
  <div class="hero-extras">
    <div v-if="frontmatter.install" class="install">
      <span class="install-label">{{ frontmatter.install.label }}</span>
      <code class="install-command"><span class="prompt">$</span> {{ frontmatter.install.command }}</code>
      <button class="install-copy" type="button" :aria-label="frontmatter.install.copy" @click="copy">
        {{ copied ? frontmatter.install.copied : frontmatter.install.copy }}
      </button>
    </div>
    <figure v-if="frontmatter.demo" class="demo">
      <img :src="withBase('/demo.gif')" :alt="frontmatter.demo.alt" width="760" />
      <figcaption>{{ frontmatter.demo.caption }}</figcaption>
    </figure>
  </div>
</template>

<style scoped>
.hero-extras {
  max-width: 760px;
  margin: 0 auto;
  padding: 0 24px 32px;
}

.install {
  display: flex;
  align-items: center;
  gap: 12px;
  margin: 8px auto 40px;
  padding: 10px 12px 10px 16px;
  max-width: 560px;
  border: 1px solid var(--vp-c-divider);
  border-radius: 10px;
  background: var(--vp-c-bg-soft);
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

.demo {
  margin: 0;
}

.demo img {
  display: block;
  width: 100%;
  height: auto;
  border: 1px solid var(--vp-c-divider);
  border-radius: 12px;
  box-shadow: 0 24px 60px -24px var(--vp-c-brand-soft), 0 2px 8px rgb(0 0 0 / 0.08);
}

.demo figcaption {
  margin-top: 10px;
  text-align: center;
  font-size: 13px;
  color: var(--vp-c-text-2);
}

@media (max-width: 560px) {
  .install-label {
    display: none;
  }
}
</style>
