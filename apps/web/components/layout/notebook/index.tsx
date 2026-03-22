import type * as PageTree from "fumadocs-core/page-tree";
import { TreeContextProvider } from "fumadocs-ui/contexts/tree";
import { Languages, Sidebar as SidebarIcon, X } from "lucide-react";
import {
  type ComponentProps,
  type FC,
  type HTMLAttributes,
  type ReactNode,
  useMemo,
} from "react";
import { cn } from "../../../lib/cn";
import { buttonVariants } from "../../ui/button";
import { LanguageToggle } from "../language-toggle";
import { LinkItem, type LinkItemType } from "../link-item";
import { LargeSearchToggle, SearchToggle } from "../search-toggle";
import { type BaseLayoutProps, renderTitleNav, useLinkItems } from "../shared";
import type { SidebarPageTreeComponents } from "../sidebar/page-tree";
import { type GetSidebarTabsOptions, getSidebarTabs } from "../sidebar/tabs";
import {
  SidebarTabsDropdown,
  type SidebarTabWithProps,
} from "../sidebar/tabs/dropdown";
import { ThemeToggle } from "../theme-toggle";
import {
  LayoutBody,
  LayoutContextProvider,
  LayoutHeader,
  LayoutHeaderTabs,
  NavbarLinkItem,
} from "./client";
import {
  Sidebar,
  SidebarCollapseTrigger,
  SidebarContent,
  SidebarDrawer,
  SidebarLinkItem,
  SidebarPageTree,
  SidebarTrigger,
  SidebarViewport,
} from "./sidebar";

export interface DocsLayoutProps extends BaseLayoutProps {
  tree: PageTree.Root;
  tabMode?: "sidebar" | "navbar";

  nav?: BaseLayoutProps["nav"] & {
    mode?: "top" | "auto";
  };

  sidebar?: SidebarOptions;

  containerProps?: HTMLAttributes<HTMLDivElement>;
}

interface SidebarOptions
  extends ComponentProps<"aside">,
    Pick<ComponentProps<typeof Sidebar>, "defaultOpenLevel" | "prefetch"> {
  components?: Partial<SidebarPageTreeComponents>;

  /**
   * Root Toggle options
   */
  tabs?: SidebarTabWithProps[] | GetSidebarTabsOptions | false;

  banner?: ReactNode | FC<ComponentProps<"div">>;
  footer?: ReactNode | FC<ComponentProps<"div">>;

  /**
   * Support collapsing the sidebar on desktop mode
   *
   * @defaultValue true
   */
  collapsible?: boolean;
}

export function DocsLayout(props: DocsLayoutProps) {
  const {
    tabMode = "sidebar",
    nav = {},
    sidebar: {
      tabs: tabOptions,
      defaultOpenLevel,
      prefetch,
      ...sidebarProps
    } = {},
    i18n = false,
    themeSwitch = {},
    tree,
  } = props;

  const navMode = nav.mode ?? "auto";
  const { menuItems, navItems } = useLinkItems(props);
  const tabs = useMemo(() => {
    if (Array.isArray(tabOptions)) {
      return tabOptions;
    }

    if (typeof tabOptions === "object") {
      return getSidebarTabs(tree, tabOptions);
    }

    if (tabOptions !== false) {
      return getSidebarTabs(tree);
    }

    return [];
  }, [tabOptions, tree]);

  function sidebar() {
    const {
      banner,
      footer,
      components,
      collapsible = true,
      ...rest
    } = sidebarProps;

    const iconLinks = menuItems.filter((item) => item.type === "icon");
    const Header =
      typeof banner === "function"
        ? banner
        : ({ className, ...props }: ComponentProps<"div">) => (
            <div
              className={cn(
                "flex flex-col gap-3 p-4 pb-2 empty:hidden",
                className,
              )}
              {...props}
            >
              {props.children}
              {banner}
            </div>
          );
    const Footer =
      typeof footer === "function"
        ? footer
        : ({ className, ...props }: ComponentProps<"div">) => (
            <div
              className={cn(
                "hidden flex-row text-fd-muted-foreground items-center border-t p-4 pt-2",
                iconLinks.length > 0 && "max-lg:flex",
                className,
              )}
              {...props}
            >
              {props.children}
              {footer}
            </div>
          );
    const viewport = (
      <SidebarViewport>
        {/* {menuItems
          .filter((item) => item.type !== "icon")
          .map((item, i, arr) => (
            <SidebarLinkItem
              key={i}
              item={item}
              className={cn("lg:hidden", i === arr.length - 1 && "mb-4")}
            />
          ))} */}

        <SidebarPageTree {...components} />
      </SidebarViewport>
    );

    return (
      <>
        <SidebarContent {...rest}>
          <Header>
            {navMode === "auto" && (
              <div className="flex justify-between">
                {renderTitleNav(nav, {
                  className: "inline-flex items-center gap-2.5 font-medium",
                })}
                {collapsible && (
                  <SidebarCollapseTrigger
                    className={cn(
                      buttonVariants({
                        color: "ghost",
                        size: "icon-sm",
                        className: "mt-px mb-auto text-fd-muted-foreground",
                      }),
                    )}
                  >
                    <SidebarIcon />
                  </SidebarCollapseTrigger>
                )}
              </div>
            )}
            {tabs.length > 0 && (
              <SidebarTabsDropdown
                options={tabs}
                className={cn(tabMode === "navbar" && "lg:hidden")}
              />
            )}
          </Header>
          {viewport}
          <Footer>
            {iconLinks.map((item, i) => (
              <LinkItem
                key={i}
                item={item}
                className={cn(
                  buttonVariants({
                    size: "icon-sm",
                    color: "ghost",
                    className: "lg:hidden",
                  }),
                )}
                aria-label={item.label}
              >
                {item.icon}
              </LinkItem>
            ))}
          </Footer>
        </SidebarContent>
        <SidebarDrawer {...rest}>
          <Header>
            <SidebarTrigger
              className={cn(
                buttonVariants({
                  size: "icon-sm",
                  color: "ghost",
                  className: "ms-auto text-fd-muted-foreground",
                }),
              )}
            >
              <X />
            </SidebarTrigger>
            {tabs.length > 0 && <SidebarTabsDropdown options={tabs} />}
          </Header>
          {viewport}
          <Footer
            className={cn(
              "hidden flex-row items-center justify-end",
              (i18n || themeSwitch.enabled !== false) && "flex",
              iconLinks.length > 0 && "max-lg:flex",
            )}
          >
            <a
              href="https://x.com/ruru_1x"
              target="_black"
              className={cn(
                buttonVariants({ size: "icon-sm", color: "ghost" }),
                "text-fd-muted-foreground",
              )}
            >
              <svg className="scale-85" fill="none" viewBox="0 0 1200 1227">
                <path
                  fill="currentColor"
                  d="M714.163 519.284 1160.89 0h-105.86L667.137 450.887 357.328 0H0l468.492 681.821L0 1226.37h105.866l409.625-476.152 327.181 476.152H1200L714.137 519.284h.026ZM569.165 687.828l-47.468-67.894-377.686-540.24h162.604l304.797 435.991 47.468 67.894 396.2 566.721H892.476L569.165 687.854v-.026Z"
                />
              </svg>
            </a>
            {iconLinks.map((item, i) => (
              <LinkItem
                key={i}
                item={item}
                className={cn(
                  buttonVariants({
                    size: "icon-sm",
                    color: "ghost",
                  }),
                  "text-fd-muted-foreground lg:hidden",
                  i === iconLinks.length - 1 && "me-auto",
                )}
                aria-label={item.label}
              >
                {item.icon}
              </LinkItem>
            ))}
            {i18n && (
              <LanguageToggle>
                <Languages className="size-4.5 text-fd-muted-foreground" />
              </LanguageToggle>
            )}
            {themeSwitch.enabled !== false &&
              (themeSwitch.component ?? (
                <ThemeToggle mode={themeSwitch.mode ?? "light-dark-system"} />
              ))}
          </Footer>
        </SidebarDrawer>
      </>
    );
  }

  return (
    <TreeContextProvider tree={tree}>
      <LayoutContextProvider
        navMode={nav.mode ?? "auto"}
        tabMode={tabMode}
        navTransparentMode={nav.transparentMode}
      >
        <Sidebar defaultOpenLevel={defaultOpenLevel} prefetch={prefetch}>
          <LayoutBody {...props.containerProps}>
            {sidebar()}
            <DocsNavbar {...props} links={navItems} tabs={tabs} />
            {props.children}
          </LayoutBody>
        </Sidebar>
      </LayoutContextProvider>
    </TreeContextProvider>
  );
}

function DocsNavbar({
  links,
  tabs,
  tabMode = "sidebar",
  sidebar: { collapsible: sidebarCollapsible = true } = {},
  searchToggle = {},
  themeSwitch = {},
  nav = {},
  i18n,
}: DocsLayoutProps & {
  links: LinkItemType[];
  tabs: SidebarTabWithProps[];
}) {
  const navMode = nav.mode ?? "auto";
  const showLayoutTabs = tabMode === "navbar" && tabs.length > 0;

  return (
    <LayoutHeader
      id="nd-subnav"
      className={cn(
        "sticky [grid-area:header] flex flex-col top-(--fd-docs-row-1) z-10 backdrop-blur-sm transition-colors data-[transparent=false]:bg-fd-background/80 layout:[--fd-header-height:--spacing(12)]",
        showLayoutTabs && "lg:layout:[--fd-header-height:--spacing(24)]",
      )}
    >
      <div
        data-header-body=""
        className="flex px-2 gap-2 h-12 md:px-4 border-b "
      >
        <div
          className={cn(
            "items-center",
            navMode === "top" && "flex flex-1",
            navMode === "auto" &&
              "hidden has-data-[collapsed=true]:md:flex max-md:flex",
          )}
        >
          {sidebarCollapsible && navMode === "auto" && (
            <SidebarCollapseTrigger
              className={cn(
                buttonVariants({
                  color: "ghost",
                  size: "icon-sm",
                }),
                "text-fd-muted-foreground data-[collapsed=false]:hidden max-md:hidden",
              )}
            >
              <SidebarIcon />
            </SidebarCollapseTrigger>
          )}
          {renderTitleNav(nav, {
            className: cn(
              "inline-flex items-center gap-2.5 font-semibold -translate-y-0.5",
              navMode === "auto" && "md:hidden",
            ),
          })}
          <div className="flex items-center gap-2 empty:hidden max-lg:hidden ml-6">
            {links
              .filter((item) => item.type !== "icon")
              .map((item, i) => (
                <NavbarLinkItem key={i} item={item} />
              ))}
          </div>
          {nav.children}
        </div>
        {searchToggle.enabled !== false &&
          (searchToggle.components?.lg ? (
            <div
              className={cn(
                "w-full my-auto max-md:hidden",
                navMode === "top" ? "rounded-xl max-w-sm" : "max-w-[240px]",
              )}
            >
              {searchToggle.components.lg}
            </div>
          ) : (
            <LargeSearchToggle
              hideIfDisabled
              className={cn(
                "w-full my-auto max-md:hidden",
                navMode === "top"
                  ? "rounded-xl max-w-sm ps-2.5"
                  : "max-w-[240px]",
              )}
            />
          ))}
        <div className="flex flex-1 items-center justify-end md:gap-2">
          {links
            .filter((item) => item.type === "icon")
            .map((item, i) => (
              <LinkItem
                key={i}
                item={item}
                className={cn(
                  buttonVariants({ size: "icon-sm", color: "ghost" }),
                  "text-fd-muted-foreground max-lg:hidden",
                )}
                aria-label={item.label}
              >
                {item.icon}
              </LinkItem>
            ))}

          <div className="flex items-center md:hidden">
            {searchToggle.enabled !== false &&
              (searchToggle.components?.sm ?? (
                <SearchToggle hideIfDisabled className="p-2" />
              ))}
            <SidebarTrigger
              className={cn(
                buttonVariants({
                  color: "ghost",
                  size: "icon-sm",
                  className: "p-2 -me-1.5",
                }),
              )}
            >
              <SidebarIcon />
            </SidebarTrigger>
          </div>

          <div className="flex items-center gap-2 max-md:hidden">
            <a
              href="https://x.com/ruru_1x"
              target="_black"
              className={cn(
                buttonVariants({ size: "icon-sm", color: "ghost" }),
                "text-fd-muted-foreground",
              )}
            >
              <svg className="scale-85" fill="none" viewBox="0 0 1200 1227">
                <path
                  fill="currentColor"
                  d="M714.163 519.284 1160.89 0h-105.86L667.137 450.887 357.328 0H0l468.492 681.821L0 1226.37h105.866l409.625-476.152 327.181 476.152H1200L714.137 519.284h.026ZM569.165 687.828l-47.468-67.894-377.686-540.24h162.604l304.797 435.991 47.468 67.894 396.2 566.721H892.476L569.165 687.854v-.026Z"
                />
              </svg>
            </a>
            {i18n && (
              <LanguageToggle>
                <Languages className="size-4.5 text-fd-muted-foreground" />
              </LanguageToggle>
            )}
            {themeSwitch.enabled !== false &&
              (themeSwitch.component ?? (
                <ThemeToggle
                  className="p-0 border-none *:rounded-md"
                  mode={themeSwitch.mode ?? "light-dark-system"}
                />
              ))}
            {/* {sidebarCollapsible && navMode === "top" && (
              <SidebarCollapseTrigger
                className={cn(
                  buttonVariants({
                    color: "secondary",
                    size: "icon-sm",
                  }),
                  "text-fd-muted-foreground rounded-full -me-1.5",
                )}
              >
                <SidebarIcon />
              </SidebarCollapseTrigger>
            )} */}
          </div>
        </div>
      </div>
      {showLayoutTabs && (
        <LayoutHeaderTabs
          data-header-tabs=""
          className="overflow-x-auto border-b px-6 h-10 max-lg:hidden"
          options={tabs}
        />
      )}
    </LayoutHeader>
  );
}
