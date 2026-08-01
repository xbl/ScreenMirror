<template>
  <TrayPanel v-if="isTrayPanel" />
  <HostShell v-else />
</template>

<script setup lang="ts">
import HostShell from './components/HostShell.vue';
import TrayPanel from './components/TrayPanel.vue';

const isTrayPanel = new URLSearchParams(window.location.search).get('tray') === '1';
</script>

<style>
@import './styles/tokens.css';
@import './styles/base.css';
</style>
