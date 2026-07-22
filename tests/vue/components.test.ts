import { mount } from '@vue/test-utils';
import DeviceInfoCallout from '../../src/components/DeviceInfoCallout.vue';
import { describe, it, expect } from 'vitest';

describe('DeviceInfoCallout', () => {
  it('renders all device fields', () => {
    const wrapper = mount(DeviceInfoCallout, {
      props: {
        device: {
          id: 'd1',
          name: 'Test',
          ip: '10.0.0.1',
          os: 'macOS',
          browser: 'Safari',
          roomId: '123456',
          sharingSessionId: 'sess-1',
        },
      },
    });
    expect(wrapper.text()).toContain('10.0.0.1');
    expect(wrapper.text()).toContain('macOS');
    expect(wrapper.text()).toContain('Safari');
    expect(wrapper.text()).toContain('123456');
  });
});