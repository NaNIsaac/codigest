<script setup lang="ts">
import { ref } from 'vue'
import { useData, withBase } from 'vitepress'

const { frontmatter } = useData()
const poster = withBase('/demo-poster.png')
const animation = withBase('/demo.gif')
const src = ref(poster)
const playing = ref(false)

// The GIF opens on an empty prompt, so the page shows a still with real output and plays the
// animation on hover, focus or click. A new query string restarts it from the first frame.
function play() {
  if (playing.value) return
  playing.value = true
  src.value = `${animation}?play=${Date.now()}`
}

function stop() {
  playing.value = false
  src.value = poster
}
</script>

<template>
  <figure v-if="frontmatter.demo" class="demo">
    <button
      class="screen"
      type="button"
      :aria-label="frontmatter.demo.alt"
      @mouseenter="play"
      @focus="play"
      @mouseleave="stop"
      @blur="stop"
      @click="playing ? stop() : play()"
    >
      <img :src="src" :alt="frontmatter.demo.alt" width="760" height="557" />
      <span v-if="!playing" class="play" aria-hidden="true"></span>
    </button>
    <figcaption>{{ frontmatter.demo.caption }}</figcaption>
  </figure>
</template>

<style scoped>
.demo {
  margin: 0;
  width: 100%;
  min-width: 0;
}

.screen {
  position: relative;
  display: block;
  box-sizing: border-box;
  width: 100%;
  max-width: 100%;
  padding: 0;
  border-radius: 12px;
  cursor: pointer;
  box-shadow: 0 20px 48px -24px rgb(0 0 0 / 0.45);
}

.screen img {
  display: block;
  width: 100%;
  height: auto;
  border: 1px solid var(--vp-c-divider);
  border-radius: 12px;
}

.play {
  position: absolute;
  right: 16px;
  bottom: 16px;
  width: 36px;
  height: 36px;
  border-radius: 50%;
  background: var(--vp-c-brand-3);
  opacity: 0.92;
  transition: transform 0.2s;
}

.play::after {
  content: '';
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-35%, -50%);
  border-left: 11px solid #fff;
  border-top: 7px solid transparent;
  border-bottom: 7px solid transparent;
}

.screen:hover .play {
  transform: scale(1.08);
}

.demo figcaption {
  margin-top: 10px;
  text-align: center;
  font-size: 13px;
  color: var(--vp-c-text-2);
}
</style>
