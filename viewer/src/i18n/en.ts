export default {
  viewer: {
    title: 'ScreenMirror Viewer',
    reinitiate: 'Re-initiate connection',
    waitingForAllow: 'Waiting for host to allow your device...',
    waitingForSource: 'Waiting for host to choose a source...',
    connected: 'Connected!',
    myDevice: 'My Device Info',
    deviceType: 'Device Type',
    deviceIp: 'Device IP',
    deviceBrowser: 'Browser',
    deviceOs: 'OS',
    connectionId: 'Connection ID',
    ipHelp: 'Verify this IP matches the one shown in the host app.',
    disconnected: 'You were disconnected',
  },
  controls: {
    play: 'Play',
    pause: 'Pause',
    quality: 'Quality',
    fullscreen: 'Fullscreen',
  },
  privacy: {
    title: 'Privacy',
    intro: 'ScreenMirror mirrors your screen over the local network. We do not need any data to do this.',
    bullet1: 'No analytics, telemetry, or third-party tracking.',
    bullet2: 'Frames are transferred peer-to-peer over WebRTC; the host does not store them.',
    bullet3: 'No data is sent to a third party.',
    close: 'Close',
  },
  player: {
    connecting: 'Connecting…',
    streaming: 'Streaming',
    disconnected: 'Connection lost',
    reconnect: 'Reconnect',
    noFrames: 'No video — reconnect?',
  },
};

export type Messages = typeof import('./en').default;
