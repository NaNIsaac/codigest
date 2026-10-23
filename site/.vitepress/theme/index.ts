import { h } from 'vue'
import type { Theme } from 'vitepress'
import DefaultTheme from 'vitepress/theme'
import HeroDemo from './HeroDemo.vue'
import HeroInstall from './HeroInstall.vue'
import HomeWorkflows from './HomeWorkflows.vue'
import './style.css'

export default {
  extends: DefaultTheme,
  enhanceApp({ app }) {
    app.component('HomeWorkflows', HomeWorkflows)
  },
  Layout: () =>
    h(DefaultTheme.Layout, null, {
      'home-hero-info-after': () => h(HeroInstall),
      'home-hero-image': () => h(HeroDemo),
    }),
} satisfies Theme
