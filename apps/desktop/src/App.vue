<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { FolderPlus, Plus, Save } from "@lucide/vue";
import { SpecxMark } from "@/components/brand";
import AppToolbar from "@/components/layout/AppToolbar.vue";
import AppSidebar from "@/components/layout/AppSidebar.vue";
import AppTabBar from "@/components/layout/AppTabBar.vue";
import SpecificationDialog from "@/components/layout/SpecificationDialog.vue";
import SettingsPage from "@/components/settings/SettingsPage.vue";
import ServicePanel from "@/components/ServicePanel.vue";
import LoginPanel from "@/components/auth/LoginPanel.vue";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from "@/components/ui/dialog";
import { useWorkspaceStore } from "@/stores/workspace";
import { useSettingsStore } from "@/stores/settingsStore";
import { useNavigationStore } from "@/stores/navigationStore";
import { usePanelResize } from "@/composables/usePanelResize";
import { useToast } from "@/composables/useToast";
import { useAppShortcuts } from "@/composables/useAppShortcuts";
import { useUiFontFamily } from "@/composables/useUiFontFamily";
import { specxApi } from "@/lib/backend/client";
import type { ResourceTreeNode } from "@/types/navigation";
import { useShortcutStore } from "@/stores/shortcutStore";
import { useAppUpdateStore } from "@/stores/appUpdateStore";
import { translate } from "@/i18n";

// Composition root: layout, UI state and runtime-neutral catalog commands.
const workspace = useWorkspaceStore();
const settings = useSettingsStore();
const navigation = useNavigationStore();
const t = translate;
const shortcuts = useShortcutStore();
const updates = useAppUpdateStore();
const { sidebarWidth, startSidebarResize } = usePanelResize();
const { message, visible, action, toast, dismissToast } = useToast();
useUiFontFamily();
const sidebar = ref<InstanceType<typeof AppSidebar>>();
const showSpecification = ref(false);
const pendingDelete = ref<ResourceTreeNode | null>(null);
const deleting = ref(false);
const pendingCloseTab = computed(() => workspace.tabs.find((tab) => tab.id === workspace.pendingCloseTabId));
useAppShortcuts(() => {
  void nextTick(() => sidebar.value?.focusSearch());
});
onMounted(() => {
  window.addEventListener("beforeunload", beforeUnload);
  void workspace.initialize();
  void updates.initialize().then(() => {
    if (updates.autoCheck) return updates.check();
  });
});
function beforeUnload(event: BeforeUnloadEvent) {
  if (!workspace.tabs.some((tab) => tab.specificationId && workspace.hasUnsavedSpecification(tab.specificationId))) return;
  event.preventDefault();
  event.returnValue = "";
}
onBeforeUnmount(() => window.removeEventListener("beforeunload", beforeUnload));
function newService() {
  settings.sidebarCollapsed = false;
  navigation.beginCreate("resource");
}
async function remove() {
  if (!pendingDelete.value || deleting.value) return;
  deleting.value = true;
  try {
    await navigation.remove(pendingDelete.value);
    pendingDelete.value = null;
  } catch (cause) {
    toast(cause instanceof Error ? cause.message : t("app.deleteFailed"), 5000);
  } finally {
    deleting.value = false;
  }
}
async function logout() {
  try {
    await specxApi.logout();
    workspace.authenticated = false;
    workspace.catalog = null;
    settings.showSettingsPage = false;
  } catch (cause) {
    toast(cause instanceof Error ? cause.message : t("app.logoutFailed"), 5000);
  }
}
async function resolvePendingClose(action: "save" | "discard" | "cancel") {
  await workspace.resolvePendingClose(action);
}
</script>

<template>
  <div v-if="workspace.loading" class="app-loading" role="status">{{ t("app.connecting") }}</div>
  <LoginPanel v-else-if="workspace.authenticated === false" />
  <main v-else-if="workspace.error && !workspace.catalog" class="app-loading gap-3">
    <h1>{{ workspace.migrationReady ? t("app.loadFailed") : t("app.storageNotReady") }}</h1>
    <p>{{ workspace.error }}</p>
    <Button variant="outline" @click="workspace.initialize()">{{ t("common.retry") }}</Button>
  </main>
  <div v-else-if="workspace.catalog" data-app-shell class="app-shell flex flex-col overflow-hidden bg-background text-foreground">
    <AppToolbar @new-service="newService" @new-specification="showSpecification = true" @logout="logout" />
    <div class="flex min-h-0 flex-1" :class="settings.appLayout === 'classic' ? 'app-layout-classic' : 'app-panel-gutter gap-1 p-1'">
      <AppSidebar v-if="!settings.sidebarCollapsed" ref="sidebar" :sidebar-width="sidebarWidth" @start-resize="startSidebarResize" @collapse="settings.sidebarCollapsed = true" @remove="pendingDelete = $event" />
      <main class="flex min-w-0 flex-1 flex-col overflow-hidden bg-background" :class="{ 'specx-panel': settings.appLayout === 'separated' }">
        <AppTabBar v-if="workspace.tabs.length || settings.showSettingsPage" />
        <SettingsPage v-if="settings.showSettingsPage" />
        <div v-else-if="workspace.activeTab && workspace.activeServiceId" class="min-h-0 flex-1 overflow-hidden p-4 sm:p-6 lg:p-8">
          <ServicePanel :catalog="workspace.catalog" :service-id="workspace.activeServiceId" :tab="workspace.activeTab" @open-specification="workspace.openSpecification" @new-specification="showSpecification = true" />
        </div>
        <div v-else class="flex min-h-0 flex-1 flex-col items-center justify-center gap-4 p-8">
          <SpecxMark :size="72" animated decorative />
          <h1 class="text-xl font-medium">{{ t("app.emptyTitle") }}</h1>
          <p class="max-w-md text-center text-sm text-muted-foreground">{{ t("app.emptyDescription") }}</p>
          <div class="flex gap-2">
            <Button
              variant="outline"
              size="sm"
              @click="
                settings.sidebarCollapsed = false;
                navigation.beginCreate('group');
              "
              ><FolderPlus class="h-3.5 w-3.5" />{{ t("app.createGroup") }}</Button
            ><Button size="sm" @click="newService"><Plus class="h-3.5 w-3.5" />{{ t("app.createService") }}</Button>
          </div>
          <div class="mt-6 text-xs text-muted-foreground">{{ [shortcuts.hint(t("settings.shortcutSearch"), "search"), shortcuts.hint(t("settings.shortcutSettings"), "settings")].join(" · ") }}</div>
        </div>
      </main>
    </div>
    <SpecificationDialog v-model:open="showSpecification" />
    <Dialog :open="updates.phase === 'installing' || updates.phase === 'restart'">
      <DialogContent :show-close-button="false" @escape-key-down.prevent @interact-outside.prevent>
        <DialogHeader
          ><DialogTitle>{{ t("updates.title") }}</DialogTitle></DialogHeader
        >
        <p role="status" class="text-sm">{{ updates.phase === "installing" ? t("updates.installing") : t("updates.restartPending") }}</p>
        <p v-if="updates.errorKey" role="alert" class="text-sm text-destructive">{{ t(updates.errorKey) }}</p>
        <DialogFooter v-if="updates.phase === 'restart'"
          ><Button @click="updates.restart">{{ t("updates.restart") }}</Button></DialogFooter
        >
      </DialogContent>
    </Dialog>
    <Dialog
      :open="!!pendingDelete"
      @update:open="
        (value) => {
          if (!value && !deleting) pendingDelete = null;
        }
      "
      ><DialogContent :show-close-button="!deleting"
        ><DialogHeader
          ><DialogTitle>{{ pendingDelete?.type === "resource" ? t("app.deleteServiceTitle") : t("app.deleteGroupTitle") }}</DialogTitle></DialogHeader
        >
        <p class="text-sm text-muted-foreground">{{ pendingDelete?.type === "resource" ? t("app.confirmDeleteService", { name: pendingDelete.label }) : t("app.confirmDeleteGroup", { name: pendingDelete?.label ?? "" }) }}</p>
        <DialogFooter
          ><Button variant="outline" :disabled="deleting" @click="pendingDelete = null">{{ t("common.cancel") }}</Button
          ><Button variant="destructive" :disabled="deleting" @click="remove">{{ deleting ? t("app.deleting") : t("common.delete") }}</Button></DialogFooter
        ></DialogContent
      ></Dialog
    >
    <Dialog :open="!!workspace.pendingCloseTabId" @update:open="(value) => !value && resolvePendingClose('cancel')">
      <DialogContent>
        <DialogHeader
          ><DialogTitle>{{ t("specification.closeWithUnsavedTitle") }}</DialogTitle></DialogHeader
        >
        <p class="text-sm text-muted-foreground">{{ t("specification.closeWithUnsavedDescription") }}</p>
        <p v-if="pendingCloseTab" class="truncate text-xs text-muted-foreground">{{ pendingCloseTab.title }}</p>
        <DialogFooter>
          <Button variant="outline" @click="resolvePendingClose('cancel')">{{ t("common.cancel") }}</Button>
          <Button variant="ghost" @click="resolvePendingClose('discard')">{{ t("specification.discardAndClose") }}</Button>
          <Button @click="resolvePendingClose('save')"><Save class="h-3.5 w-3.5" />{{ t("specification.saveAndClose") }}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
  <div v-if="visible" role="status" class="toast">
    <span>{{ message }}</span
    ><button
      v-if="action"
      class="ml-3 underline"
      @click="
        action.onClick();
        dismissToast();
      "
    >
      {{ action.label }}</button
    ><button class="ml-3" :aria-label="t('app.dismissToast')" @click="dismissToast">×</button>
  </div>
</template>
