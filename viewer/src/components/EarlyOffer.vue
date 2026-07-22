<template>
  <div style="display: none" data-component="early-offer"></div>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue';

const props = defineProps<{ ws: WebSocket | null; roomId: string }>();
const emit = defineEmits<{ (e: 'ready'): void }>();

const pc = ref<RTCPeerConnection | null>(null);
let started = false;

function send(msg: { type: string; payload?: unknown }) {
  const rs = props.ws?.readyState;
  console.log('[early-offer] send()', msg.type, 'ws state:', rs);
  if (props.ws && rs === 1) {
    try {
      props.ws.send(JSON.stringify(msg));
    } catch (e) {
      console.error('[early-offer] send threw:', e);
    }
  } else {
    console.error('[early-offer] cannot send: ws not open');
  }
}

async function startNegotiation() {
  if (started || !props.ws) return;
  started = true;

  // Ask the host for a real WebRTC video track. ICE candidates are gathered
  // into the SDP so the existing signaling path remains unchanged.
  pc.value = new RTCPeerConnection({ iceServers: [] });
  pc.value.addTransceiver('video', { direction: 'recvonly' });
  pc.value.ontrack = (event) => {
    const stream = event.streams[0] ?? new MediaStream([event.track]);
    console.log('[early-offer] video track received', event.track.kind);
    window.dispatchEvent(new CustomEvent('viewer-stream', { detail: stream }));
    emit('ready');
  };

  pc.value.onicecandidate = (e) => {
    if (e.candidate) {
      const candidate = e.candidate.candidate;
      const match = candidate.match(/candidate:\S+\s+\d+\s+\S+\s+\d+\s+(\S+)\s+\d+/);
      if (match?.[1] && !match[1].includes(':')) {
        window.dispatchEvent(new CustomEvent('viewer-ip', { detail: match[1] }));
      }
      send({ type: 'ICE_CANDIDATE', payload: { candidate: e.candidate.toJSON() } });
    } else {
      // null candidate = end-of-candidates; not all hosts need this.
    }
  };
  pc.value.addEventListener('iceconnectionstatechange', () => {
    console.log('[early-offer] ice:', pc.value?.iceConnectionState);
  });
  pc.value.addEventListener('icegatheringstatechange', () => {
    console.log('[early-offer] ice gathering:', pc.value?.iceGatheringState);
  });
  pc.value.addEventListener('connectionstatechange', () => {
    console.log('[early-offer] connectionState:', pc.value?.connectionState);
  });

  // Tell the host we're a viewer.
  send({ type: 'USER_ENTER', payload: { username: 'viewer' } });
  console.log('[early-offer] USER_ENTER sent');

  // Wait past server 500ms throttle before sending OFFER.
  await new Promise((r) => setTimeout(r, 700));

  // createOffer puts PC into have-local-offer so setRemoteDescription(answer)
  // is well-defined.
  const offer = await pc.value.createOffer({ offerToReceiveAudio: false, offerToReceiveVideo: true });
  await pc.value.setLocalDescription(offer);
  await new Promise<void>((resolve) => {
    if (pc.value?.iceGatheringState === 'complete') {
      resolve();
      return;
    }
    const onGathering = () => {
      if (pc.value?.iceGatheringState === 'complete') {
        pc.value.removeEventListener('icegatheringstatechange', onGathering);
        resolve();
      }
    };
    pc.value?.addEventListener('icegatheringstatechange', onGathering);
    window.setTimeout(() => {
      pc.value?.removeEventListener('icegatheringstatechange', onGathering);
      resolve();
    }, 3000);
  });
  console.log('[early-offer] ice gathering complete:', pc.value.iceGatheringState);
  const sdp = pc.value.localDescription?.sdp ?? offer.sdp ?? '';
  send({ type: 'OFFER', payload: { sdp } });
  console.log('[early-offer] OFFER sent (real SDP, len=' + sdp.length + ')');
}

onMounted(() => {
  if (!props.ws) return;
  console.log('[early-offer] mounted, ws state:', props.ws?.readyState, props.ws?.url);

  if (props.ws.readyState === 1) {
    void startNegotiation();
  } else {
    props.ws.addEventListener('open', () => void startNegotiation(), { once: true });
  }

  props.ws.addEventListener('error', (e) => {
    console.log('[early-offer] WS error:', (e as ErrorEvent).message ?? 'unknown');
  });
  props.ws.addEventListener('close', (e) => {
    console.log('[early-offer] WS close code=' + e.code + ' reason=' + (e.reason || '(none)'));
  });

  props.ws.addEventListener('message', (e) => {
    const text = e.data.toString();
    console.log('[early-offer] recv msg, data len:', text.length);
    if (text.includes('"ANSWER"')) {
      try {
        const m = JSON.parse(text);
        if (m.payload?.sdp && pc.value) {
          console.log('[early-offer] applying ANSWER, pc state=' + pc.value.signalingState);
          pc.value
            .setRemoteDescription({ type: 'answer', sdp: m.payload.sdp })
            .then(() => console.log('[early-offer] ANSWER set ok'))
            .catch((err) => console.error('[early-offer] ANSWER setRemoteDescription failed:', err?.message ?? err));
        }
      } catch (err) {
        console.error('[early-offer] ANSWER parse err', err);
      }
    } else if (text.includes('"ICE_CANDIDATE"')) {
      try {
        const m = JSON.parse(text);
        if (m.payload?.candidate && pc.value) {
          pc.value.addIceCandidate(m.payload.candidate).catch(() => {});
        }
      } catch {}
    }
  });
});
</script>
