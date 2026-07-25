import type { Ref } from 'vue';

export type PermissionModalRef = {
  checkAndShow: () => Promise<void>;
};

export const PermissionModalKey: unique symbol = Symbol('PermissionModal');

export type PermissionModalInjectionKey = typeof PermissionModalKey;

export type ProvidedPermissionModal = Ref<PermissionModalRef | null>;
