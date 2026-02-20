# SweetPad `Build & Run (Launch)` 执行流程（可复刻为 CLI）

本文档基于 SweetPad VSCode Extension 当前代码实现，梳理命令 **“SweetPad: Build & Run (Launch)”**（命令 ID：`sweetpad.build.launch`）的**真实调用链**与其底层 `xcodebuild` / `xcrun` 命令组合方式。目标是让你仅凭本文即可实现一个等价的 CLI 工具（交互式或纯参数式）。

> 术语约定：
> - “workspace/xcworkspace” 在 SweetPad 里既可能是 **`.xcworkspace`**（Xcode 工程）也可能是 **`Package.swift`**（SPM 工程）。代码里用 `detectWorkspaceType()` 通过 “路径是否以 `Package.swift` 结尾” 来区分。
> - “destination” 统一指运行目标：macOS、本机模拟器、真机（iOS/watchOS/tvOS/visionOS）。

---

## 1. 顶层入口：命令注册与函数入口

### 1.1 VSCode command

命令在 `package.json` 中声明：

- `sweetpad.build.launch` → **SweetPad: Build & Run (Launch)**

在 `src/extension.ts` 中注册：

- `command("sweetpad.build.launch", launchCommand)`

### 1.2 直接入口函数

`src/build/commands.ts`：

- `launchCommand(context, item?)` → `context.buildManager.launchCommand(item, { debug: false })`

即：**Launch = 非 debug 模式**（debug 的另一条命令是 `sweetpad.debugger.debuggingLaunch`，这里不展开；本文末尾会写差异点）。

---

## 2. 核心流程：`BuildManager.launchCommand()`

源码：`src/build/manager.ts` 的 `launchCommand(item, { debug })`。

整体流程可以概括为：

1. 解析/选择 Xcode workspace（`.xcworkspace` 或 `Package.swift`）
2. 解析/选择 scheme
3. （可选）生成/修复 `buildServer.json`（xcode-build-server）
4. 解析/选择 build configuration
5. 解析/选择 destination（模拟器/真机/Mac）
6. 组装并执行 `xcodebuild … build`
7. 根据 destination 类型执行 “安装 + 启动”

下面按步骤展开到具体命令。

---

## 3. 输入解析与选择（scheme / configuration / destination / workspace）

### 3.1 workspace（`.xcworkspace` 或 `Package.swift`）

入口：`askXcodeWorkspacePath(context)`（`src/build/utils.ts`）

决策顺序：

1. **设置项** `sweetpad.build.xcodeWorkspacePath`：
   - 可为绝对路径，或相对 workspace root 的路径（会 `path.join(workspaceRoot, relative)`）。
2. **缓存**（workspace state）`build.xcodeWorkspacePath`：
   - 由用户上次选择后写入。
3. 自动探测（深度 4）：
   - 搜索所有以 `.xcworkspace` 结尾的路径
   - 以及所有名为 `Package.swift` 的文件路径
   - 若只有 1 个且 `autoselect=true`，直接用；否则进入交互选择。

> CLI 复刻建议：
> - `--workspace <path>` 显式指定。
> - 不指定时执行同样的递归探测；如多个候选，交互选择。

### 3.2 scheme

入口：`askSchemeForBuild(context, { xcworkspace, ignoreCache? })`（`src/build/utils.ts`）

决策顺序：

1. 如果是从 TreeItem 触发（`item?.scheme`），则直接用该 scheme。
2. 缓存（workspace state）`build.xcodeScheme`：
   - 若存在且未设置 `ignoreCache=true`，直接用。
3. 否则列出 schemes 并让用户选择；选择后写回 `build.xcodeScheme`。

schemes 的来源：`getSchemes({ xcworkspace })`（`src/common/cli/scripts.ts`）

- **SPM（`Package.swift`）**：
  1. 先尝试 `swift package dump-package`（在 `Package.swift` 所在目录执行）
  2. 从 dump 的 JSON 中提取：
     - products 里 `executable` 或 `library`
     - 以及 `targets` 里 `type === "executable"`
     - 最后兜底用 `packageInfo.name`
  3. 若失败，fallback 到后续方式
- **Xcode（`.xcworkspace`）**：
  1. 若设置 `sweetpad.system.customXcodeWorkspaceParser=true`，尝试自定义解析器（解析 `.xcworkspace` & `.xcodeproj` 文件）拿 schemes
  2. fallback：`xcodebuild -list -json -workspace <xcworkspace>`，取 `workspace.schemes`（或 `project.schemes`）

> CLI 复刻建议：
> - `--scheme <name>` 显式指定。
> - 不指定时：按上述逻辑列出 scheme（优先 SPM dump-package；Xcode 可选 workspace parser；最后 xcodebuild -list）。

### 3.3 configuration

入口：`askConfiguration(context, { xcworkspace })`（`src/build/utils.ts`）

决策顺序：

1. 设置项 `sweetpad.build.configuration` 若存在，直接用。
2. 缓存（workspace state）`build.xcodeConfiguration` 若存在，直接用。
3. 否则执行 `askConfigurationBase()`（`src/common/askers.ts`）：
   - 获取 configurations：`getBuildConfigurations({ xcworkspace })`
     - SPM：直接返回 `Debug` / `Release`
     - Xcode：优先 workspace parser（如果启用）；否则从 `xcodebuild -list -json` 或解析 projects 得到配置列表
   - 若配置集正好是 `Debug` + `Release`，则**默认选择 `Debug`**（避免每次都问用户）
   - 其他情况弹出选择
4. 最终将选择写回缓存 `build.xcodeConfiguration`

> CLI 复刻建议：
> - `--configuration Debug|Release|...`
> - 若未指定：按 SweetPad 规则在 `Debug/Release` 时默认 `Debug`。

### 3.4 destination（模拟器/真机/Mac）

入口：`askDestinationToRunOn(context, { scheme, configuration, sdk, xcworkspace })`（`src/build/utils.ts`）

决策顺序：

1. 拉取 destinations 列表：`context.destinationsManager.getDestinations({ mostUsedSort: true })`
   - 模拟器：`xcrun simctl list --json devices`
   - 真机：`xcrun devicectl list devices --json-output <tmp> --timeout 10`
   - 额外包含 “My Mac”（按当前 CPU 架构 arm64/x86_64）
2. 若缓存（workspace state）`build.xcodeDestination` 存在，且能在当前 destinations 列表中匹配到同 `id` + `type`，则直接复用（不弹选择）。
3. 计算可支持的平台（用于 UI 上把 “Supported” 放上面）：
   - 调用 `getBuildSettingsToAskDestination()`：
     - 运行 `xcodebuild -showBuildSettings -scheme <scheme> -configuration <configuration> [-sdk <sdk>] [-workspace <xcworkspace>] [-derivedDataPath …] -json`
     - 解析 `SUPPORTED_PLATFORMS`（例如 `iphonesimulator iphoneos`）作为 “supported platforms”
     - 注意：当 scheme 对应多个 targets 时，这里会返回 `null`（SweetPad 选择 “不做过滤”）
4. 交互选择目标 destination 后写回缓存，并更新：
   - 使用次数统计：`build.xcodeDestinationsUsageStatistics`
   - 最近使用列表：`build.xcodeDestinationsRecent`

> CLI 复刻建议：
> - `--destination` 推荐设计为两层：
>   - `--platform iphonesimulator|iphoneos|macosx|...`
>   - `--udid <simulator-udid|device-udid>`（macOS 可无）
> - 或提供 `--device "<name>"` 之类的 fuzzy 选择，内部仍落到 UDID。
> - 若未指定：执行上面的 list + prompt，并将上一次选择持久化到本地配置（对应 SweetPad 的 workspace state）。

---

## 4. 构建阶段：`xcodebuild … build`

构建由 `buildApp(terminal, options)` 完成（`src/build/manager.ts`）。

### 4.1 destination string（传给 `xcodebuild -destination`）

由 `getXcodeBuildDestinationString({ destination })`（`src/build/utils.ts`）生成：

- Simulator：
  - `platform=iOS Simulator,id=<UDID>[,arch=x86_64]`
  - `arch=x86_64` 仅在设置项 `sweetpad.build.rosettaDestination=true` 时启用（用于 Apple Silicon 上强制 x86_64 模拟器构建）
- Device：
  - `platform=iOS,id=<UDID>`
- macOS：
  - `platform=macOS,arch=<arm64|x86_64>`

### 4.2 `xcodebuild` 可执行文件

由 `getXcodeBuildCommand()`（`src/common/cli/scripts.ts`）决定：

- 设置项 `sweetpad.build.xcodebuildCommand`（支持 `${env:VAR}` 展开）
- 否则默认 `xcodebuild`

### 4.3 组装参数（核心模板）

SweetPad 最终执行的是：

```
xcodebuild \
  [BUILD_SETTING=VALUE ...] \
  -scheme <scheme> \
  -configuration <configuration> \
  -destination "<destinationRaw>" \
  -resultBundlePath <bundlePath> \
  [-derivedDataPath <derivedDataPath>] \
  [-allowProvisioningUpdates] \
  [-workspace <xcworkspace>] \
  build \
  [...additionalArgs]
```

其中：

- `bundlePath`：每次构建会在扩展的 storage 目录下创建/清理（用于 `-resultBundlePath`）
  - CLI 复刻可用 `mktemp -d` 或 `<project>/.sweetpad/bundle/<scheme>` 等策略
- `derivedDataPath`：来自设置项 `sweetpad.build.derivedDataPath`（可相对路径 → 以 workspace root 展开）
- `-workspace`：**仅在 `.xcworkspace` 类型项目**时传；SPM 项目不传 `-workspace`，而是把 `cwd` 切到 `Package.swift` 的目录
- `-allowProvisioningUpdates`：由 `sweetpad.build.allowProvisioningUpdates`（默认 true）控制
- 额外参数 `sweetpad.build.args`：
  - 可以是普通 flag 形式（如 `-skipMacroValidation`）
  - 也可以是 `KEY=VALUE` 形式（会作为 build setting 注入到命令开头）
  - 也允许包含 `build/clean/test` 这类 action（SweetPad 会去重）
  - 去重逻辑是 “**后出现的覆盖先出现**”
- 传给 `xcodebuild` 的环境变量 `sweetpad.build.env`：
  - 值为 `null` 表示 unset（内部转换为 `undefined`）

### 4.4 可选：`xcbeautify` 管道

若满足：

- `sweetpad.build.xcbeautifyEnabled=true` 且
- `which xcbeautify` 能找到

则执行逻辑等价于：

```
set -o pipefail; xcodebuild ... | xcbeautify
```

> CLI 复刻建议：
> - 提供 `--xcbeautify/--no-xcbeautify` 或自动检测。
> - 使用 `pipefail` 保证 xcodebuild 失败时 CLI 仍返回非 0。

### 4.5 可选：xcode-build-server（buildServer.json）

SweetPad 在 Launch 流程里至少会触发一次 `generateBuildServerConfigOnBuild({ scheme, xcworkspace })`：

触发条件：

- `sweetpad.xcodebuildserver.autogenerate=true`（默认 true）
- `xcode-build-server` 可用（支持自定义路径 `sweetpad.xcodebuildserver.path`）
- 且 `buildServer.json` 不存在/不可读/字段不匹配/引用路径不存在

执行命令（概念上）：

- Xcode：`xcode-build-server config -workspace <xcworkspace> -scheme <scheme>`
- SPM：`xcode-build-server config -scheme <scheme>`（在 Package.swift 目录执行）

随后会尝试重启 SourceKit-LSP：`swift.restartLSPServer`（VSCode 命令）。

> CLI 复刻建议：
> - 这块与 “能否 build & run” 无强依赖，可先忽略；之后有需求再做成 `--generate-build-server-config` 子命令。

---

## 5. 运行阶段：根据 destination 类型执行安装与启动

SweetPad 的 Launch 在 build 完成后分三类：

1. macOS：直接执行可执行文件
2. Simulator：`simctl boot/install/launch`
3. Device：`devicectl install + process launch`

### 5.1 共同前置：获取 build settings（定位 appPath / bundleId / executable）

SweetPad 通过 `xcodebuild -showBuildSettings -json` 获取路径与 bundle id。

关键点：

- `getBuildSettingsToLaunch()` 会处理 “同一个 scheme 有多个 target” 的情况：
  1. 先拿到 `-showBuildSettings` 输出的多个 target settings
  2. 再解析 `.xcscheme` XML 的 `LaunchAction` 找到 “要 launch 的 target”
  3. 在 settings 列表里选择对应 target 的那一份
  4. 若解析失败则兜底用第一份

对于 CLI 来说，你需要至少得到：

- `.app` bundle 路径（iOS 模拟器/真机安装用）
- `PRODUCT_BUNDLE_IDENTIFIER`（启动用）
- macOS 场景需要可执行文件路径（`TARGET_BUILD_DIR + EXECUTABLE_PATH`）

### 5.2 macOS destination：直接运行可执行文件

步骤：

1. 获取 build settings（sdk 固定 `macosx`）：
   - `xcodebuild -showBuildSettings ... -sdk macosx -json`
2. 取 `executablePath` 并校验文件存在
3. 运行：
   - `/<path/to/executable> <launchArgs...>`，并注入 `launchEnv`

### 5.3 Simulator destination：`simctl`

输入：

- simulator UDID
- `.app` bundle path
- bundle identifier
- launch args（`sweetpad.build.launchArgs`）
- launch env（`sweetpad.build.launchEnv`）

步骤与命令：

1. 确保 simulator 已 boot：
   - `xcrun simctl boot <SIMULATOR_UDID>`
2. 打开 Simulator.app：
   - `open -a Simulator`（默认，会前置到前台）
   - 或 `open -g -a Simulator`（若 `sweetpad.build.bringSimulatorToForeground=false`）
3. 安装 app：
   - `xcrun simctl install <SIMULATOR_UDID> <APP_PATH>`
4. 启动 app（核心）：
   - `xcrun simctl launch --console-pty --terminate-running-process <SIMULATOR_UDID> <BUNDLE_ID> [<launchArgs...>]`
   - 若是 debug 模式（不是本文的 Launch，但可复刻）：会额外加 `--wait-for-debugger`
5. 向 app 传 env 的方式：
   - **把每个 env 变量改名为 `SIMCTL_CHILD_<KEY>`**，再作为 `xcrun` 的环境变量传入

### 5.4 Device destination：`devicectl`

输入：

- device UDID
- `.app` bundle path
- bundle identifier
- launch args / launch env

步骤与命令：

1. 安装 app：
   - `xcrun devicectl device install app --device <DEVICE_UDID> <APP_PATH>`
2. 判定 `--console` 是否可用：
   - 运行 `xcrun xcodebuild -version`，解析出 major 版本号
   - 若 major >= 16，则 `devicectl device process launch` 增加 `--console`
3. 启动 app（并写 JSON 输出）：
   - `xcrun devicectl device process launch [--console] --json-output <JSON_PATH> --terminate-existing --device <DEVICE_UDID> <BUNDLE_ID> [<launchArgs...>]`
4. 向 app 传 env 的方式：
   - **把每个 env 变量改名为 `DEVICECTL_CHILD_<KEY>`**，再作为 `xcrun` 的环境变量传入
5. 读取 `<JSON_PATH>`：
   - 若 `info.outcome !== "success"`，打印 `result` 并返回
   - 否则输出 PID：`result.process.processIdentifier`

> CLI 复刻建议：
> - `devicectl` 是 SweetPad 的真机能力基础（list devices / install / launch 都依赖它）。
> - 若找不到 `devicectl`（典型是没装完整 Xcode 或 `xcrun` 找不到组件），CLI 应给出 “请安装 Xcode / 选择正确的 Xcode 版本” 之类提示。

---

## 6. SweetPad 在 Launch 过程中维护的“状态”（供 CLI 做持久化参考）

SweetPad 使用 VSCode workspaceState（存储前缀 `sweetpad.`）做缓存。对 CLI 等价实现，可以用：

- `<project>/.sweetpad/state.toml`

关键状态：

- `build.xcodeWorkspacePath`：上次选择的 workspace 路径
- `build.xcodeScheme`：默认 scheme
- `build.xcodeConfiguration`：默认 configuration
- `build.xcodeDestination`：默认 destination（`{ id, type, name }`）
- `build.xcodeDestinationsUsageStatistics`：destination 使用计数（用于排序）
- `build.xcodeDestinationsRecent`：最近使用的 destination 列表
- `build.lastLaunchedApp`：最后一次 launch 的 app 上下文（mac/sim/device 类型 + 路径 + bundleId 等）

---

## 7. CLI 复刻：推荐的最小“可用”设计

### 7.1 建议命令形态

最小子命令：

- `tool detect`：列出可用 workspace 候选（`.xcworkspace` / `Package.swift`）
- `tool schemes --workspace <path>`
- `tool configs --workspace <path>`
- `tool destinations`：列出 simulators / devices / mac
- `tool launch [flags]`：等价 `sweetpad.build.launch`

### 7.2 `launch` 的 flags（建议）

- 选择输入：
  - `--workspace <path>`
  - `--scheme <name>`
  - `--configuration <name>`（默认遵循 SweetPad：Debug/Release 时取 Debug）
  - `--destination "<type>:<udid>"` 或 `--platform` + `--udid`
- 构建相关：
  - `--xcodebuild <path>`（对应 `sweetpad.build.xcodebuildCommand`）
  - `--derived-data <path>`（对应 `sweetpad.build.derivedDataPath`）
  - `--arch arm64|x86_64`（对应 `sweetpad.build.arch`）
  - `--[no-]allow-provisioning-updates`
  - `--xcbeautify|--no-xcbeautify`
  - `--build-arg ...`（可重复；对应 `sweetpad.build.args`）
  - `--build-env KEY=VALUE`（可重复；对应 `sweetpad.build.env`）
- 运行相关：
  - `--arg ...`（可重复；对应 `sweetpad.build.launchArgs`）
  - `--env KEY=VALUE`（可重复；对应 `sweetpad.build.launchEnv`）
  - `--[no-]bring-simulator-to-foreground`
  - `--rosetta-destination`（对 simulator 添加 `arch=x86_64`）

### 7.3 伪代码（等价 Launch）

```
workspace = resolveWorkspace()
scheme = resolveScheme(workspace)
configuration = resolveConfiguration(workspace)
destination = resolveDestination(workspace, scheme, configuration)
destinationRaw = toXcodebuildDestinationString(destination, rosettaFlag)

build(workspaceType, xcodebuildPath, scheme, configuration, destinationRaw, derivedDataPath, allowProvisioningUpdates, buildArgs, buildEnv, resultBundlePath)

settings = getBuildSettingsToLaunch(workspaceType, scheme, configuration, sdkForDestination(destination), derivedDataPath)
appPath/executablePath + bundleId = extract(settings)

if destination is simulator:
  simctl boot if needed
  open Simulator.app
  simctl install appPath
  simctl launch with SIMCTL_CHILD_ env prefix
elif destination is device:
  devicectl install appPath
  devicectl process launch with DEVICECTL_CHILD_ env prefix + json output
elif destination is macOS:
  run executablePath with env + args
```

---

## 8. 与 “Debug launch” 的差异（可选）

虽然本文聚焦 `sweetpad.build.launch`（debug=false），但若你实现 CLI，通常也会提供 `debug` 模式。SweetPad debug 相关差异点主要有：

- Simulator 启动参数增加：`xcrun simctl launch ... --wait-for-debugger`
- `xcodebuild` 强制注入 build settings：
  - `GCC_GENERATE_DEBUGGING_SYMBOLS=YES`
  - `ONLY_ACTIVE_ARCH=YES`

---

## 9. 你实现 CLI 时最容易踩的坑（按 SweetPad 经验）

1. `xcodebuild -showBuildSettings -json` 有时会在 JSON 前后输出 warning/额外文本：需要 “从输出中截取第一个 `{`/`[` 到最后一个 `}`/`]`” 的容错解析。
2. scheme 可能包含多个 target：要稳定定位真正的可启动 target，需要读取 `.xcscheme` 的 `LaunchAction`（SweetPad 已实现相应解析逻辑）。
3. Simulator env 传递必须用 `SIMCTL_CHILD_` 前缀，否则 app 内拿不到环境变量。
4. Device env 传递必须用 `DEVICECTL_CHILD_` 前缀（`devicectl` 的约定）。
5. 真机 `devicectl` 的 `--console` 需要 Xcode 16+；低版本要跳过该参数。

