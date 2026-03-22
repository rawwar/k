<script setup lang="ts">
import DefaultTheme from 'vitepress/theme'
import SettingsPanel from './SettingsPanel.vue'
import ContinueReading from './ContinueReading.vue'
import ProgressIndicator from './ProgressIndicator.vue'

const { Layout } = DefaultTheme
</script>

<template>
  <Layout>
    <template #nav-bar-content-after>
      <ContinueReading />
      <SettingsPanel />
    </template>
    <template #sidebar-nav-before>
      <ProgressIndicator mode="sidebar" />
    </template>
    <template #doc-before>
      <ProgressIndicator mode="chapter" />
    </template>
  </Layout>
</template>
