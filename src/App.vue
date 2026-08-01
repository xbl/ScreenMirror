<template>
  <TrayPanel v-if="isTrayPanel" />
  <SourcePicker v-else-if="isSourcePicker" standalone />
  <HostShell v-else />
</template>

<script setup lang="ts">
import HostShell from './components/HostShell.vue';
import TrayPanel from './components/TrayPanel.vue';
import SourcePicker from './components/SourcePicker.vue';

const params = new URLSearchParams(window.location.search);
const isTrayPanel = params.get('tray') === '1';
const isSourcePicker = params.get('source-picker') === '1';
</script>

<style>
@import './styles/tokens.css';
@import './styles/base.css';
</style>
