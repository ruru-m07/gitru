# Changelog

## [0.0.0-beta.5](https://github.com/ruru-m07/gitru/compare/v0.0.0-beta.4...v0.0.0-beta.5) (2026-04-26)


### Features

* enhance inbox tab display with item count and visual separator ([c3cbc45](https://github.com/ruru-m07/gitru/commit/c3cbc457e0167e93bb5f158dbac18436fa8f2e2c))
* implement basic multi tab support ([6ee78d4](https://github.com/ruru-m07/gitru/commit/6ee78d4bfcd53d77c9d0201fe6e9d2a2f98a7f87))
* implement drag-and-drop tab reordering functionality ([1c91cc6](https://github.com/ruru-m07/gitru/commit/1c91cc6815ce53297400a6dfb8b9e551292dedf5))
* implement session navigation management ([ed53f72](https://github.com/ruru-m07/gitru/commit/ed53f725a8f3ef08cf20357b6f5d4cad41c5ed68))
* implement tab switching functionality with keyboard shortcuts and MRU support ([69e1d09](https://github.com/ruru-m07/gitru/commit/69e1d09965916848f4d6579a14ed4d82673a3119))
* make tabs starter UI in mail layout ([ccb445c](https://github.com/ruru-m07/gitru/commit/ccb445c883993ede9770209b9ce8476727b71d33))
* make tabs starter UI in mail layout ([baad6f7](https://github.com/ruru-m07/gitru/commit/baad6f7ef72795eb495a783678bbfabf7d0b33cf))
* update UI components for improved layout and accessibility ([d06dca7](https://github.com/ruru-m07/gitru/commit/d06dca71353a1cc3ff634970612d85fbdf48dac6))


### Bug Fixes

* default selected repo when create new tab ([d8a82ff](https://github.com/ruru-m07/gitru/commit/d8a82ff8d4bb89d7fc940ecfe4ea7c832e1ff874))
* diff view bg color to match theme ([7d9ccda](https://github.com/ruru-m07/gitru/commit/7d9ccda165dff665dcbfea3ca6a13baffe8ad2ea))
* diff view bg color to match theme ([b068623](https://github.com/ruru-m07/gitru/commit/b0686238b42847290f9f40af93d9e01eabfbe2c6))
* implement custom resize pannel for fixed size pannel ([5fc46ae](https://github.com/ruru-m07/gitru/commit/5fc46ae9a3e75b7e4e257ec2a7cfdf2c1cbb1a5a))
* implement custom resize pannel for fixed size pannel ([faeb0a2](https://github.com/ruru-m07/gitru/commit/faeb0a229be5288c69dcc2d01c417dd923092da2))
* title bar hover state UI fixes ([9a1e349](https://github.com/ruru-m07/gitru/commit/9a1e349cf7009e12a34a625ee3e45ceeeb381853))
* update title and prevent default actions in CustomTitleBar ([eeca848](https://github.com/ruru-m07/gitru/commit/eeca848e7a9f57012db7b10f442fe9e0c22938fa))


### Miscellaneous Chores

* release 0.0.0-beta.5 ([7d97c73](https://github.com/ruru-m07/gitru/commit/7d97c739a9cbd87c1a778be3e626783f22ced12a))

## [0.0.0-beta.4](https://github.com/ruru-m07/gitru/compare/v0.0.0-beta.3...v0.0.0-beta.4) (2026-03-23)


### Features

* add -moz-user-select style for better text selection prevention in Firefox ([3ac961f](https://github.com/ruru-m07/gitru/commit/3ac961f23c156f10446f4ec56dbe8440dba4e5fa))
* add -webkit-user-select to prevent text selection on hover ([2988780](https://github.com/ruru-m07/gitru/commit/2988780bdb835259b579bf35e2857260e4877dcd))
* add repository cloning and initialization features ([ef6e036](https://github.com/ruru-m07/gitru/commit/ef6e036ea8c5409db8541289e97dbb45072ac319))
* add user-select none style to prevent text selection ([1fb5b4a](https://github.com/ruru-m07/gitru/commit/1fb5b4a2e6324bf0b491187a5519ba57cc4c19e7))
* **diff:** add language guessing utility and parsing enhancements ([65ea0c0](https://github.com/ruru-m07/gitru/commit/65ea0c03a54caaa44b8d22ee8d2ffd8070a8d20e))
* **diff:** integrate @pierre/diffs for enhanced diff functionality and refactor related structures ([32a0a0c](https://github.com/ruru-m07/gitru/commit/32a0a0cc7f322ab047d77f8cf657c7c0264f2c83))
* enhance UI and functionality in various components ([3e1be5f](https://github.com/ruru-m07/gitru/commit/3e1be5f7dd19a0ecb284128b44b79d1b873b0b2d))
* enhance VirtualizedFileList with file selection and keyboard navigation features ([8f6043c](https://github.com/ruru-m07/gitru/commit/8f6043c652c3d7bfeea4ab4bb66c76b787102259))
* implement apply patch per block of hunks ([11a5117](https://github.com/ruru-m07/gitru/commit/11a5117bb90813bc30f9c68f1584bbb33d44cfdc))
* improve onboarding flow ([13905ae](https://github.com/ruru-m07/gitru/commit/13905ae2c8082e4ea1629779fe741e8c203f926b))
* optimize file selection handling in useAppStore and VirtualizedFileList ([30f0a1b](https://github.com/ruru-m07/gitru/commit/30f0a1b00644bb3151e2d4e19c1b06572d1028c5))
* remove unused SVG code from GitruBorderedSmallSmileSVG component ([b508a14](https://github.com/ruru-m07/gitru/commit/b508a14bbfc36d4dcff49f28c236c1b190efe861))
* update bulk operation on working tree to filter specific files ([bf00b04](https://github.com/ruru-m07/gitru/commit/bf00b04500d6aceb57e684177aeafc4e8fe6a060))
* update web UI and shader ([ce980ea](https://github.com/ruru-m07/gitru/commit/ce980eabc249191ab3aa0a269f881d2dc2b73fdf))


### Bug Fixes

* allow clippy warning for too many arguments in patch retrieval functions ([5e0c30d](https://github.com/ruru-m07/gitru/commit/5e0c30d13518164007b5b04601bc6c148c4c1f31))
* **ci:** update frontend lint testing in same test workflow ([47050e4](https://github.com/ruru-m07/gitru/commit/47050e405027927918901d92f2e3c317ec0f9023))
* desktop build issues due to linting errors ([4cedc72](https://github.com/ruru-m07/gitru/commit/4cedc72de6ea939faa405131f78a6f4a719d4fc9))
* minor style issues ([c1e58de](https://github.com/ruru-m07/gitru/commit/c1e58de2b5377b12a967d580eafe6ebd40af8ea4))
* refine CSS user-select property and update default file names in DiffArea ([7bbe8f0](https://github.com/ruru-m07/gitru/commit/7bbe8f065ff2472cc7798ffb664ca523603a513b))


### Miscellaneous Chores

* release 0.0.0-beta.4 ([64db42b](https://github.com/ruru-m07/gitru/commit/64db42bb028d845dfccd605c8df694404c1e73ef))

## [0.0.0-beta.3](https://github.com/ruru-m07/gitru/compare/v0.0.0-beta.2...v0.0.0-beta.3) (2026-03-05)


### Features

* add tags and file change statistics to history commit list ([511a6f9](https://github.com/ruru-m07/gitru/commit/511a6f90f7ee8e6dd61228850b0ec71070039393))
* enhance diff and commit handling with history support ([8b595a1](https://github.com/ruru-m07/gitru/commit/8b595a16e89d2fde5635e2c3569c3db8c14c985a))
* implement the support for asset diff ([ef046fa](https://github.com/ruru-m07/gitru/commit/ef046fab9be8309a78e0b478a4b64b2b07ff437d))


### Bug Fixes

* address review comments - security fixes, bug fixes, resource cleanup ([9dbd484](https://github.com/ruru-m07/gitru/commit/9dbd484b7b6455754cb96b3ca70fe0d21cab99c9))
* update updater base URL to use the correct release endpoint ([e0b5974](https://github.com/ruru-m07/gitru/commit/e0b597476eba26915e1549f796b7f15dd9d9bf85))


### Miscellaneous Chores

* release 0.0.0-beta.3 ([810093b](https://github.com/ruru-m07/gitru/commit/810093b4e4496d47711d92dacd90249861a99deb))

## [0.0.0-beta.2](https://github.com/ruru-m07/gitru/compare/v0.0.0-beta.1...v0.0.0-beta.2) (2026-02-28)


### Features

* add StashState and related hooks for stash management functionality ([8660dc3](https://github.com/ruru-m07/gitru/commit/8660dc38f4777403621b2d912df708f77acd5c66))
* enhance stash functionality and diff retrieval ([20b6e50](https://github.com/ruru-m07/gitru/commit/20b6e5060b3fe7b0b284acccbcb88714a690bca5))
* enhance VirtualizedFileList with sectionMode prop and improve status bar styling ([e1c5bca](https://github.com/ruru-m07/gitru/commit/e1c5bcaeffdff1a409aaa91e9783a2fef396b3ac))
* implement stash management commands and UI icons ([ab977f2](https://github.com/ruru-m07/gitru/commit/ab977f27420bc79550ecc060c607a903f7ab7633))
* update versioning and add new update process ([521218b](https://github.com/ruru-m07/gitru/commit/521218bb92d47c7f06c1c432c0c04d18b22ed25e))


### Bug Fixes

* standardize casing for project name and update package details in configuration files ([4dddac8](https://github.com/ruru-m07/gitru/commit/4dddac8a7d8abc4c110f680d280b9bb3fc145bb4))
* streamline checkout process and improve error handling in branch switching ([ab37ced](https://github.com/ruru-m07/gitru/commit/ab37cedd2275bdb10efa2332b4d56663b1730c9c))


### Miscellaneous Chores

* release 0.0.0-beta.2 ([210faaf](https://github.com/ruru-m07/gitru/commit/210faaf131b049e810ca664047dc84abf163c8cb))

## 0.0.0-beta.1 (2026-02-21)


### Features

* ability to make commits ([04d1279](https://github.com/ruru-m07/gitru/commit/04d12798254aa0c4c3bb03179d9e8e6f030eb3f5))
* add cache layer on all command ([2017078](https://github.com/ruru-m07/gitru/commit/2017078d9b74162da9c842c68d1aaccf0718e36b))
* add cache layer on all command ([039f1c9](https://github.com/ruru-m07/gitru/commit/039f1c9800b52762841f1e1656ad1f83ba0598bc))
* add conflict handling and improve branch management ([d3166d7](https://github.com/ruru-m07/gitru/commit/d3166d7116950a48a91f26de024b87efe5a200a8))
* add development icons and update configuration for Tauri ([c86b98e](https://github.com/ruru-m07/gitru/commit/c86b98e5182e772491db80252624018d7c6dc930))
* add diff functionality for files ([63c4424](https://github.com/ruru-m07/gitru/commit/63c4424d9a69eb43b451de7895d194b0f2d1c21e))
* add file status filters to ListFileChanges component ([310d39d](https://github.com/ruru-m07/gitru/commit/310d39d75538657eb45984369453f6464404e79d))
* add git pull functionality and related hooks ([d5bb593](https://github.com/ruru-m07/gitru/commit/d5bb59379a9fbb5204f87addd6b9d930ac2a9754))
* add motion library and integrate motion components in App and Waitlist ([7a6c482](https://github.com/ruru-m07/gitru/commit/7a6c48216501eb7660e31e3ab5b1d50992106514))
* add motion library for animations and update dependencies in package.json and bun.lock ([4392636](https://github.com/ruru-m07/gitru/commit/43926364e6ef59516cbf13d5d36d31beaaf5daf1))
* add new UI components including Meter, NumberField, Pagination, PreviewCard, Sheet, Slider, Spinner, Toast, ToggleGroup, and Toolbar ([6c2224c](https://github.com/ruru-m07/gitru/commit/6c2224cec20fb2c7dc2e0adee970fdecb9d5ab18))
* add onCallBack support for action items in command view ([445ead8](https://github.com/ruru-m07/gitru/commit/445ead8a931fce6bc5713827efa36a917ec7620d))
* add openVscode command and integrate with VirtualizedFileList for opening files in VS Code ([efc6706](https://github.com/ruru-m07/gitru/commit/efc6706b54f79d1b4a85a10499516c3475cbcf8a))
* add support for making commits ([bf3611d](https://github.com/ruru-m07/gitru/commit/bf3611d45a991a974675030cbb83cbb3174f2fcc))
* add support for remote branches checkout ([0b00f29](https://github.com/ruru-m07/gitru/commit/0b00f296a3dc46c6efef65dae6bec6f3a13ac986))
* add support for untracked file status in ListFileChanges component ([d22b759](https://github.com/ruru-m07/gitru/commit/d22b759eb754080b16cd34f620587a0f7acc02b3))
* add SVG components for split and unified views ([63c4424](https://github.com/ruru-m07/gitru/commit/63c4424d9a69eb43b451de7895d194b0f2d1c21e))
* add switch repo in action pannel and UI fixes ([02e2e01](https://github.com/ruru-m07/gitru/commit/02e2e01329780828aaa2828075c16b6c0ccfbea7))
* add switch repo in action pannel and UI fixes ([5e5284c](https://github.com/ruru-m07/gitru/commit/5e5284c133fbc0a48ee809e5663b299c05043d2b))
* add time utility functions ([63c4424](https://github.com/ruru-m07/gitru/commit/63c4424d9a69eb43b451de7895d194b0f2d1c21e))
* cleanup hooks and setup query mutations ([f60c5fc](https://github.com/ruru-m07/gitru/commit/f60c5fca77e14456b1008f99059a5c6689b82267))
* define types for Git operations ([63c4424](https://github.com/ruru-m07/gitru/commit/63c4424d9a69eb43b451de7895d194b0f2d1c21e))
* enhance command item callbacks to close context after execution ([387eaa6](https://github.com/ruru-m07/gitru/commit/387eaa6873477b6ee5cef1cfd76d7c6b4bcaf546))
* enhance command list with virtualization support and improve branch list display ([7a793e8](https://github.com/ruru-m07/gitru/commit/7a793e8ec249e34bd13b4e2b868a3119e0e6ff31))
* enhance commit button with branch name display and truncate styling ([f6bdd90](https://github.com/ruru-m07/gitru/commit/f6bdd9073d2f9c4cadbdf7b3f97eb9e6f78b17ee))
* enhance commit history retrieval ([63c4424](https://github.com/ruru-m07/gitru/commit/63c4424d9a69eb43b451de7895d194b0f2d1c21e))
* enhance CustomTitleBar and DiffViewer components with improved styling and functionality ([2cb4c7b](https://github.com/ruru-m07/gitru/commit/2cb4c7b0e48cd625f6f977eeb35a7343274c247f))
* enhance diff view functionality with new settings and status icons ([9bc6154](https://github.com/ruru-m07/gitru/commit/9bc615460e77ba4ddaf8714b29f73051ec157c06))
* enhance diff viewer settings with new options and integrate WorkerPoolProvider ([69aad1b](https://github.com/ruru-m07/gitru/commit/69aad1bafe27ea76a7dd87406edb11e100621692))
* enhance DiffViewer options with theme and disable file headercreateTauriStorage ([a190cb5](https://github.com/ruru-m07/gitru/commit/a190cb5ba9680d8b6c191477f80e2faae5632630))
* Enhance Git command interfaces and data structures ([1be38d0](https://github.com/ruru-m07/gitru/commit/1be38d01e9abc33be07175e693d810ef1366bb96))
* Enhance Git functionality and UI improvements ([27dad50](https://github.com/ruru-m07/gitru/commit/27dad502631d06aeca42d8d04753cee059419dd7))
* enhance git repository handling with improved error reporting and logging ([935cbcf](https://github.com/ruru-m07/gitru/commit/935cbcfbf7b2b5905b901ccb2c09c4c3e3e0f492))
* enhance git restore functionality with error handling and improve UI components ([668a5a3](https://github.com/ruru-m07/gitru/commit/668a5a33ec5866bdebb986b9549a4e6a24cdaa6a))
* enhance repository selection UI with context menu and refactor file change display ([13f612d](https://github.com/ruru-m07/gitru/commit/13f612d8ed425d1fb2cf419ce63a0496a9485665))
* enhance status bar with new icons and improve repository origin handling ([4611a7a](https://github.com/ruru-m07/gitru/commit/4611a7ab298f51b3429d7187eccc64505e3b779a))
* enhance status filter UI with clear option in ListFileChanges component ([29baadc](https://github.com/ruru-m07/gitru/commit/29baadc1256e427abb31c908045ee93b940981f2))
* expose CreateBranchProps interface for better accessibility ([3de8a85](https://github.com/ruru-m07/gitru/commit/3de8a85283e772ac92197ce755de5f2fba6e67a3))
* **git:** add git commands for add, remove, discard, and commit functionality ([2560924](https://github.com/ruru-m07/gitru/commit/256092454b3ceeef7d8986c1d5b28d0e301addae))
* implement command panel with navigation and command execution features ([71808dc](https://github.com/ruru-m07/gitru/commit/71808dcf74e2d756ce153d264672a5a18cbb3b7e))
* implement core feature in statusbar ([a56a78d](https://github.com/ruru-m07/gitru/commit/a56a78d8540600ff19ae57410a93af67c1c7b5b9))
* implement file status management and status options ([42efccd](https://github.com/ruru-m07/gitru/commit/42efccd46187485267d1ec5e1b736abcdf459f0b))
* implement file status retrieval and enhance repository state management ([8aa35c5](https://github.com/ruru-m07/gitru/commit/8aa35c5477e00ac5951cab07bc0a24910f5d6b7d))
* implement Git branch management ([63c4424](https://github.com/ruru-m07/gitru/commit/63c4424d9a69eb43b451de7895d194b0f2d1c21e))
* Implement repository management and author extraction functionality ([294c038](https://github.com/ruru-m07/gitru/commit/294c038b91ef593c39cc894e414709ebbf5d1d63))
* implement search query matching for file paths in Git routes ([ef97d71](https://github.com/ruru-m07/gitru/commit/ef97d7128a9ebfd18e5ea3890384a51de8d52a7f))
* implement search query matching for file paths in Git routes ([9d9f7fd](https://github.com/ruru-m07/gitru/commit/9d9f7fd8469887c3dfb021b56756f01ca1c1576b))
* implement status of files view only ([a37cb7c](https://github.com/ruru-m07/gitru/commit/a37cb7c17311495cadf2ec34bb093581a226ec8b))
* implement support for watching ([8a2c2d5](https://github.com/ruru-m07/gitru/commit/8a2c2d54509eb5fc4172a0c8df47f41eb5510b7f))
* implement switch branches ([7237139](https://github.com/ruru-m07/gitru/commit/7237139ff96401112f98fb61210abf2c7e0a7780))
* implement theme switching functionality ([ec38c5b](https://github.com/ruru-m07/gitru/commit/ec38c5b8e31c7a3d9d53ea3d325f83fb3bb99820))
* implement theme switching functionality and update color names in the UI ([243db30](https://github.com/ruru-m07/gitru/commit/243db30a84f408c625ff450a12efdcff26c1def9))
* implement update channel management and updater functionality ([c89d821](https://github.com/ruru-m07/gitru/commit/c89d821a211a230e3a819f632a109cbef58d0653))
* implement vesper-light theme and enhance scroll area functionality ([73e9126](https://github.com/ruru-m07/gitru/commit/73e9126c26e0452e91093ce65d86a1bdcee01c4b))
* integrate new command box component and update diff display ([93e24fe](https://github.com/ruru-m07/gitru/commit/93e24fe0ebc99fbaeb4b8ac994561035d7aabf9c))
* integrate tooltip functionality into sidebar items and enhance tooltip component ([018ae73](https://github.com/ruru-m07/gitru/commit/018ae7354bb931e64d674e30c135d6e616a3b1c9))
* integrate updater plugin and update configurations ([78e8782](https://github.com/ruru-m07/gitru/commit/78e878254a0449fb4366ea676562581223c891e2))
* invalidate current branch and status ahead/behind on successful push ([be02731](https://github.com/ruru-m07/gitru/commit/be027318216743f9a6a3f0716fc742a15278b3ec))
* rebrand the software name ([e5c426e](https://github.com/ruru-m07/gitru/commit/e5c426ee8f1bd21f673a005ede4a01a4ea332089))
* Refactor HistoryGraph and GraphLane components for improved readability and organization ([a2a7fce](https://github.com/ruru-m07/gitru/commit/a2a7fce53300454c48bc248e3d6d93856b751cb9))
* Refactor useAppStore hooks for improved readability and performance ([1a2ec21](https://github.com/ruru-m07/gitru/commit/1a2ec2108846e45550b6b25e3919493f258de969))
* reorganize SVG components and update import paths ([a44c37e](https://github.com/ruru-m07/gitru/commit/a44c37ed9a19fceb24fbd728da820f880810f6ef))
* replace invalidateAll with push in MainActionBar and add loading indicator ([258e7b6](https://github.com/ruru-m07/gitru/commit/258e7b6611fefe003f9106f6f2687a0fec416852))
* setp type safe ipc piplines ([ca44bcc](https://github.com/ruru-m07/gitru/commit/ca44bcc6af00c9d3a304ea8dc0e06867287c21f0))
* setup desktop frontend ([7718529](https://github.com/ruru-m07/gitru/commit/7718529bd45b769c5fa009525a4f3352ed101eb2))
* setup diff view of modified files ([44e92f6](https://github.com/ruru-m07/gitru/commit/44e92f6be2be22429871b04e0f9b9361441ee381))
* setup ipc and watcher base ([d3cfa2a](https://github.com/ruru-m07/gitru/commit/d3cfa2afd9613f99a456233dd2fc52b03fe9e3da))
* setup logger to have runtime logs ([f6866ba](https://github.com/ruru-m07/gitru/commit/f6866ba3bb3b9a8fe38e715a7171d817b507d5ce))
* setup new app icons ([8699cb0](https://github.com/ruru-m07/gitru/commit/8699cb0be7fc2e9dea2de239104507a0b4922611))
* setup post scripts for typegen ([2e84a8a](https://github.com/ruru-m07/gitru/commit/2e84a8aa0b1c37309060932c1d278a6bcf1cfccc))
* setup routeing and fix side navbar ([affe4b5](https://github.com/ruru-m07/gitru/commit/affe4b582322fb835ec90da6248dd7f7f2a80862))
* setup status bar UI ([bac3377](https://github.com/ruru-m07/gitru/commit/bac3377e8bc46273685d833cba5097216fe29e76))
* setup tanstack router instant of react router dom ([bbb44f2](https://github.com/ruru-m07/gitru/commit/bbb44f250843f33628cf0f96f39d6796e38f9f48))
* setup tauri storage plugin ([9f02de8](https://github.com/ruru-m07/gitru/commit/9f02de8b177904bd6929c530ade29106591995ba))
* stable diff view ([de5f863](https://github.com/ruru-m07/gitru/commit/de5f863e8687c368807f2a26de098798d452f756))
* support for multiple themes ([0e596b1](https://github.com/ruru-m07/gitru/commit/0e596b179635f336cdbf7e26ad133f04f28f37f2))
* **ui:** add SVG components for empty git diff, split, and unified views ([2560924](https://github.com/ruru-m07/gitru/commit/256092454b3ceeef7d8986c1d5b28d0e301addae))
* **ui:** add TypeScript configuration files for the UI package ([1ce9dfe](https://github.com/ruru-m07/gitru/commit/1ce9dfe7e578c4be3a11f287b69b87182ed9392a))
* update app icons and enhance AheadBadge logic ([26d236a](https://github.com/ruru-m07/gitru/commit/26d236a9d79ffeb982877680ffdbbab3454bfa45))
* update application icons for rebranding ([1a42eb9](https://github.com/ruru-m07/gitru/commit/1a42eb9110e7800ae592a008488b6f0b163b2c28))
* update command bindings and types for improved Git functionality ([2339f45](https://github.com/ruru-m07/gitru/commit/2339f45310bbca97428e9083265a4018a0ca6e36))
* update default JSON formatter to Prettier, refactor repository state management, and enhance diff generation logic ([f70a5d7](https://github.com/ruru-m07/gitru/commit/f70a5d7bdc1d4fa3c5f70f621aafb2d6b27e3937))
* update highlighter initialization logic and enhance status bar with current status ([9ad839f](https://github.com/ruru-m07/gitru/commit/9ad839f5da0d3d77b925a607e6c857c1d9afbd3e))
* update project structure and dependencies, add new icons, and improve clear script ([e972b2d](https://github.com/ruru-m07/gitru/commit/e972b2d3b892bb2e09cd785aa6785226516ff8c1))
* Update Tauri bindings and schemas, add diff functionality ([d075093](https://github.com/ruru-m07/gitru/commit/d075093481b056fcebe9a8f6f27c2fee11b90dd5))
* Update Tauri command bindings and schemas ([a171530](https://github.com/ruru-m07/gitru/commit/a171530284a1ce9cdb90e763b64dfc0b1318efb9))
* Update Tauri command bindings and types ([fa0dedb](https://github.com/ruru-m07/gitru/commit/fa0dedb37dc570057cd744adb8b80c825dabd32b))
* update to new app logo ([8e57449](https://github.com/ruru-m07/gitru/commit/8e57449320decc44eef13f50f222a7459248e83e))
* update to new app logo ([c9de5aa](https://github.com/ruru-m07/gitru/commit/c9de5aa381e1869d9c273a4f9dd4979ef2cfce9c))


### Bug Fixes

* buold errors ([a5d0075](https://github.com/ruru-m07/gitru/commit/a5d0075bfeb24083661cc930d10ec2cf51cfaf23))
* diff view component and deps ([8bb57f4](https://github.com/ruru-m07/gitru/commit/8bb57f43cd276c3b244c2fb0b319277e7b1ec708))
* improve the virtualized file list ([37914c3](https://github.com/ruru-m07/gitru/commit/37914c331b6894632a70e8a6a7ca373287f3d0d4))
* improve ui and setup new icons ([055569a](https://github.com/ruru-m07/gitru/commit/055569abbb69d7a32272ca1f3df7d99e76f13014))
* lint warnings and build check ([b739830](https://github.com/ruru-m07/gitru/commit/b739830bf6b03d479c071638fac3af9122bf3933))
* linting and formate the codebase ([ed292e2](https://github.com/ruru-m07/gitru/commit/ed292e2ea843e8ed015686fea4b2880ac39b95be))
* **mac:** improved dmg image alignment and size ([c125e3a](https://github.com/ruru-m07/gitru/commit/c125e3ae9a0d3ea18eccd9a3ebbc632b2248d683))
* make some cleanup on pipelines ([04bf599](https://github.com/ruru-m07/gitru/commit/04bf59971f41b28deacca4084baf818c9741c7c3))
* minor layout and rendering issues ([13d938b](https://github.com/ruru-m07/gitru/commit/13d938b5554155af5a64b750a4eb16e7053f9860))
* minor UI fixes and improvments ([a7d86a5](https://github.com/ruru-m07/gitru/commit/a7d86a5ef576fbe28f5846c7a20391a68204cbd1))
* rename App.css to app.css for case sensitivity ([466b298](https://github.com/ruru-m07/gitru/commit/466b298b18be5f1161b7bb97b3b1ca0bbf35f7f9))
* reuse the create commit logic ([ed43d47](https://github.com/ruru-m07/gitru/commit/ed43d470f811d8bf46e2e88949aad997986470a2))
* setup batter ssh handeling on git push ([856a08d](https://github.com/ruru-m07/gitru/commit/856a08d5a53ff8084db09d131fb8a15e0ac30989))
* setup diff package and command package ([6719785](https://github.com/ruru-m07/gitru/commit/67197857b962041709bed01091d7da59af8c4c6f))
* spell typo in toast message ([d4e6cf0](https://github.com/ruru-m07/gitru/commit/d4e6cf063d6a4a26d8cd1b836df5d10faff87f20))
* status bar badges and functional stuff ([de2cbc8](https://github.com/ruru-m07/gitru/commit/de2cbc87b4c259d7854deb5b34b8c81e0dd4fbea))
* stop tracking routeTree.gen.ts ([8dbde7f](https://github.com/ruru-m07/gitru/commit/8dbde7f541c7241496b5665b637ca3f87a6bb131))
* update @types/react dependency to use catalog and clean up tsconfig paths ([f7491e2](https://github.com/ruru-m07/gitru/commit/f7491e28381cc886708ff9002280633d85d67777))
* update class names and durations for UI components ([9c8d43f](https://github.com/ruru-m07/gitru/commit/9c8d43f7dfca2f9706f2d2b803a071d1543e8a45))
* update file status retrieval logic in DiffBoxBody component ([82fabf1](https://github.com/ruru-m07/gitru/commit/82fabf16604f85f22e631277050182893465d989))
* update GitHub Actions workflow to trigger on release and clean up unused steps ([abf3fcc](https://github.com/ruru-m07/gitru/commit/abf3fcc2e38c7f38a0bc2db34499fc9cbe9ffcbd))
* update OriginBadge styles and enhance ListFileChanges layout ([9ea7aee](https://github.com/ruru-m07/gitru/commit/9ea7aeea2e9a61cc3249ed39133a0471e4fff5f5))
* update resizable component ([37d9e97](https://github.com/ruru-m07/gitru/commit/37d9e975187d82c2d8cdacc0d8a7cd771df1b781))
* update resizable component ([5680199](https://github.com/ruru-m07/gitru/commit/5680199e129f1e5e0709cdfb334d16546410d5e7))
* update tanstackRouter configuration and remove generated routeTree file ([6108dce](https://github.com/ruru-m07/gitru/commit/6108dce622b6a5512a72817c4d6026de17ab72db))
* update Tauri configuration to reference version from package.json ([956d820](https://github.com/ruru-m07/gitru/commit/956d82005bb81af16b4b06ec7ce8fbe56e1a249a))
* update version in package.json to 0.0.0 ([8fd9411](https://github.com/ruru-m07/gitru/commit/8fd9411564e5b75401b12f8ee3064b968da52f90))


### Performance Improvements

* improve diff render ([8f30151](https://github.com/ruru-m07/gitru/commit/8f30151522658a5fcf5519dc7d5ef2b94d3279c1))


### Miscellaneous Chores

* release 0.0.0-beta.1 ([8d2d6af](https://github.com/ruru-m07/gitru/commit/8d2d6afdb012e21413e49702648c3342b0d41dd2))
