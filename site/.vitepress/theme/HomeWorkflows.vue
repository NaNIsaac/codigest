<script setup lang="ts">
import { computed } from 'vue'
import { useData, withBase } from 'vitepress'

type Workflow = { title: string; details: string; commands: string[]; link: string }

const { frontmatter } = useData()
const workflows = computed<Workflow[]>(() => frontmatter.value.workflows ?? [])
</script>

<template>
  <div class="workflows">
    <article v-for="w in workflows" :key="w.title" class="workflow">
      <h3>{{ w.title }}</h3>
      <p>{{ w.details }}</p>
      <pre><code><template v-for="(c, i) in w.commands" :key="i"><span class="prompt" aria-hidden="true">$ </span>{{ c }}{{ i < w.commands.length - 1 ? '\n' : '' }}</template></code></pre>
      <a :href="withBase(w.link)">{{ frontmatter.workflowsMore }} →</a>
    </article>
  </div>
</template>

<style scoped>
.workflows {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(min(100%, 260px), 1fr));
  gap: 16px;
  margin-top: 24px;
}

.workflow {
  display: flex;
  flex-direction: column;
  padding: 20px;
  border: 1px solid var(--vp-c-divider);
  border-radius: 10px;
  background: var(--vp-c-bg-soft);
}

.vp-doc .workflow h3 {
  margin: 0 0 8px;
  font-size: 16px;
  line-height: 1.4;
}

.vp-doc .workflow p {
  flex: 1;
  margin: 0 0 14px;
  font-size: 14px;
  line-height: 1.6;
  color: var(--vp-c-text-2);
}

.workflow pre {
  margin: 0 0 14px;
  padding: 10px 12px;
  white-space: pre-wrap;
  border-radius: 6px;
  background: var(--vp-code-block-bg);
}

.vp-doc .workflow pre code {
  padding: 0;
  font-size: 13px;
  line-height: 1.6;
  color: var(--vp-c-text-1);
  background: none;
}

.prompt {
  color: var(--vp-c-brand-1);
  user-select: none;
}

.vp-doc .workflow a {
  font-size: 14px;
  font-weight: 500;
  text-decoration: none;
}
@media (max-width: 640px) {
  .workflow { padding: 16px; }
  .vp-doc .workflow pre code { font-size: 14px; }
  .vp-doc .workflow a { display: inline-block; padding-block: 8px; }
}
</style>
