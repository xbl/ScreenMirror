export default {
  viewer: {
    title: 'Screenmirror Viewer',
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
    notAllowed: 'You were not allowed to connect',
    disconnected: 'You were disconnected',
    unknown: 'An unknown error occurred',
  },
  controls: {
    play: 'Play',
    pause: 'Pause',
    quality: 'Quality',
    fullscreen: 'Fullscreen',
  },
  privacy: {
    title: 'Privacy',
    intro: 'Screenmirror mirrors your screen over the local network. We do not need any data to do this.',
    bullet1: 'No analytics, telemetry, or third-party tracking.',
    bullet2: 'Frames are transferred peer-to-peer over WebRTC; the host does not store them.',
    bullet3: 'No data is sent to a third party.',
    close: 'Close',
  },
};

export type Messages = typeof import('./en').default;
