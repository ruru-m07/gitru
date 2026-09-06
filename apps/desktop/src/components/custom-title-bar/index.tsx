import { type CustomTitleBarProps } from "./constants";
import { NavigationButtons } from "./navigation-buttons";
import { TabList } from "./tab-list";
import { useCustomTitleBar } from "./use-custom-title-bar";

const CustomTitleBar = ({ restrictedPaths = [] }: CustomTitleBarProps) => {
  const {
    pathname,
    activeTabId,
    navigationState,
    goBack,
    goForward,
    navigate,
    repositories,
    isRootShellMode,
    sensors,
    orderedTabIds,
    orderedTabs,
    tabById,
    draggingTabId,
    hoveredTabId,
    setHoveredTabId,
    isTabDragInProgress,
    suppressClickTabIdRef,
    handleActivateTab,
    handleCreateTab,
    handleCloseTab,
    handleDragStart,
    handleDragOver,
    handleDragEnd,
    handleDragCancel,
    isTabSwitcherOpen,
    tabSwitcherTabIds,
    tabSwitcherIndex,
  } = useCustomTitleBar();

  if (restrictedPaths.includes(pathname)) {
    return null;
  }

  return (
    <div
      className="h-(--main-custom-header-height) relative mr-1 flex items-center pl-4 select-none z-10"
      data-tauri-drag-region
      style={{
        // @ts-expect-error - ¯\_(ツ)_/¯
        WebkitAppRegion: "drag",
      }}
    >
      <div
        className="absolute inset-x-0 flex min-w-0 items-center"
        data-tauri-drag-region="deep"
        style={{
          // @ts-expect-error - ¯\_(ツ)_/¯
          WebkitAppRegion: "drag",
          paddingLeft: "90px",
        }}
      >
        <NavigationButtons
          activeTabId={activeTabId}
          canGoBack={Boolean(navigationState?.can_go_back)}
          canGoForward={Boolean(navigationState?.can_go_forward)}
          isRootShellMode={isRootShellMode}
          goBack={goBack}
          goForward={goForward}
          navigate={navigate}
        />
        <div className="-translate-x-3 flex h-[calc(var(--main-custom-header-height)-0px)] min-w-0 flex-1 items-center">
          <TabList
            sensors={sensors}
            orderedTabIds={orderedTabIds}
            orderedTabs={orderedTabs}
            tabById={tabById}
            activeTabId={activeTabId}
            draggingTabId={draggingTabId}
            hoveredTabId={hoveredTabId}
            setHoveredTabId={setHoveredTabId}
            isTabDragInProgress={isTabDragInProgress}
            suppressClickTabIdRef={suppressClickTabIdRef}
            repositories={repositories}
            handleActivateTab={handleActivateTab}
            handleCreateTab={handleCreateTab}
            handleCloseTab={handleCloseTab}
            handleDragStart={handleDragStart}
            handleDragOver={handleDragOver}
            handleDragEnd={handleDragEnd}
            handleDragCancel={handleDragCancel}
            isTabSwitcherOpen={isTabSwitcherOpen}
            tabSwitcherTabIds={tabSwitcherTabIds}
            tabSwitcherIndex={tabSwitcherIndex}
          />
        </div>
      </div>
    </div>
  );
};

export default CustomTitleBar;
