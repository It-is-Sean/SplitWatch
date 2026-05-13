# Split Watch Rust 教学文档

## 1. 项目是什么

`Split Watch` 是一个用 Rust 编写的终端多 pane 命令看板，CLI 名称是 `swatch`。

它的目标不是做成 `tmux` 或 `zellij` 这样的终端复用器，而是做一个轻量的、直接运行 shell 命令并周期刷新结果的仪表盘。

适合的场景包括：

- 观察 `nvidia-smi`
- 观察日志
- 观察 `git status`
- 看磁盘占用
- 跟踪训练脚本
- 观察进程和实验输出

它的关键特点是：

- 一个 pane 对应一个命令
- 每个 pane 独立定时刷新
- pane 之间互不阻塞
- 同一 pane 内命令绝不重叠执行
- 支持 preset、theme、变量、resume
- 使用 PTY 执行命令，尽量保留真实终端行为

---

## 2. 技术栈

### 2.1 核心依赖

项目依赖定义在 [Cargo.toml](/Users/sean/Code/splitwatch/Cargo.toml:1)。

主要依赖：

- `clap`
  - CLI 参数解析
- `ratatui`
  - TUI 渲染
- `crossterm`
  - 键盘、鼠标、alternate screen、raw mode
- `portable-pty`
  - 用 PTY 方式运行命令
- `serde` + `toml`
  - preset / theme 文件读写
- `directories`
  - XDG 配置目录和 state 目录
- `regex`
  - 变量替换和 ANSI 解析辅助
- `anyhow`
  - 错误处理

### 2.2 为什么选 Rust

这个项目很适合 Rust，原因是：

- 状态较多，但状态之间关系明确
- 有大量“运行态”与“持久化态”的转换
- 要求退出时能正确关闭子进程
- 要求 UI 与命令执行逻辑分离
- 要避免并发数据竞争

Rust 在这里的优势是：

- 类型系统能把状态机约束住
- 模块边界清晰
- 单二进制分发方便
- 运行开销低

---

## 3. 当前目录结构

项目核心目录如下：

```text
splitwatch/
├── Cargo.toml
├── README.md
├── examples/
│   ├── custom-theme.toml
│   ├── gpu-dev.toml
│   ├── logs.toml
│   └── train-debug.toml
├── docs/
│   └── split-watch-rust-tutorial.md
└── src/
    ├── main.rs
    ├── lib.rs
    ├── cli.rs
    ├── layout.rs
    ├── preset.rs
    ├── theme.rs
    ├── vars.rs
    ├── app.rs
    ├── app/
    │   ├── state.rs
    │   ├── input.rs
    │   └── runner.rs
    ├── tui.rs
    └── tui/
        ├── actions.rs
        ├── ansi.rs
        ├── dashboard.rs
        ├── helpers.rs
        ├── modals.rs
        ├── status.rs
        └── widgets.rs
```

这个结构体现了一条重要原则：

- `app/` 负责状态与行为
- `tui/` 负责渲染与交互表现
- `preset/theme/vars` 负责配置层

---

## 4. 启动链路

### 4.1 `main.rs`

[src/main.rs](/Users/sean/Code/splitwatch/src/main.rs:1) 很薄，只负责启动库入口。

这是 Rust CLI 项目常见做法：

- `main.rs` 只放启动代码
- 业务逻辑尽量放进 `lib.rs` 和各模块

### 4.2 `lib.rs`

[src/lib.rs](/Users/sean/Code/splitwatch/src/lib.rs:1) 做两件事：

- 暴露模块
- `pub use cli::run;`

这让整个应用从 `cli::run()` 开始。

### 4.3 `cli.rs`

[src/cli.rs](/Users/sean/Code/splitwatch/src/cli.rs:1) 是程序入口控制器。

职责：

- 解析 CLI 参数
- 决定是：
  - `list`
  - `edit`
  - `resume`
  - `load preset`
  - `-n N`
- 处理 `--theme`、`--theme-file`、`--accent`
- 处理 `--var key=value`
- 构造最终 `App`
- 调用 `run_tui(app)`

这里的一个重要设计点是：

- CLI 不直接操作 TUI 细节
- 它只负责“启动前”的决策和装配

---

## 5. 配置层设计

### 5.1 `preset.rs`

[src/preset.rs](/Users/sean/Code/splitwatch/src/preset.rs:1)

职责：

- 定义 `Preset` 和 `PanePreset`
- TOML 读写
- 默认 preset 路径解析
- `resume.toml` 路径解析
- preset 保存/加载
- preset 列表与编辑路径

关键路径规则：

- preset:
  - `$XDG_CONFIG_HOME/split-watch/presets/`
  - fallback `~/.config/split-watch/presets/`
- resume:
  - `$XDG_STATE_HOME/split-watch/resume.toml`
  - fallback `~/.local/state/split-watch/resume.toml`

这是 Rust CLI 工具中很标准的 XDG 结构。

### 5.2 `theme.rs`

[src/theme.rs](/Users/sean/Code/splitwatch/src/theme.rs:1)

职责：

- 定义 `ThemeFile`
- 定义运行态 `Theme`
- 读取内置 theme
- 读取自定义 theme 文件
- 把十六进制颜色转换成 `ratatui::style::Color`

当前 theme 里已经包含：

- `accent`
- `accent_muted`
- `foreground`
- `muted`
- `background`
- `panel`
- `border`
- `border_focused`
- `success`
- `long_running`
- `warning`
- `error`

最近新增的 `long_running` 很重要：

- 不再把“命令超时运行”的颜色硬编码在 widget 里
- 把它正式纳入主题系统

### 5.3 `vars.rs`

[src/vars.rs](/Users/sean/Code/splitwatch/src/vars.rs:1)

职责：

- 变量名校验
- `{{var}}` 替换
- 检测缺失变量
- 检测必填空变量

这一层是纯逻辑模块，非常适合单元测试。

---

## 6. 应用状态层

### 6.1 `app/state.rs`

[src/app/state.rs](/Users/sean/Code/splitwatch/src/app/state.rs:1)

这是整个应用的状态核心。

关键结构：

- `App`
- `PaneState`
- `Mode`
- `TextInput`
- `Toast`

#### `App`

它是全局运行态，包含：

- 所有 panes
- 当前 focus
- 当前 mode
- 各类输入框状态
- unresolved vars
- 当前 theme
- preset 路径
- global pause
- toast
- 是否应退出

#### `PaneState`

这是单个 pane 的状态模型，包含：

- `title`
- `cmd`
- `interval_ms`
- `paused`
- `running`
- `run_started_at`
- `long_running_latched`
- `last_long_running_ms`
- `last_started`
- `last_finished`
- `last_exit_code`
- `last_error`
- `output`
- `scroll`
- `next_run`
- `pending_run_once`
- `child`

这是最近迭代里最有价值的一块之一。

为什么要加：

- `run_started_at`
  - 用来判断是否超过 interval 还没结束
- `long_running_latched`
  - 一旦触发 long run，右上角 interval 会保持红色
- `last_long_running_ms`
  - 用户修改 interval 时，要用它判断“新的 interval 是否已经大于上次卡顿时长”

这体现了 Rust 项目很典型的做法：

- 不把复杂行为散在 UI 里
- 而是先把状态显式建模

### 6.2 `app/input.rs`

[src/app/input.rs](/Users/sean/Code/splitwatch/src/app/input.rs:1)

职责：

- 键盘事件处理
- 鼠标事件处理
- pane 焦点移动
- modal 进入/退出
- inline command 编辑
- interval 调整
- preset 保存
- delete confirm

最近几个关键交互都在这里收敛了：

- `Enter` 进入 pane 内联 `>` 编辑
- 进入已有命令 pane 的内联编辑时，先暂停
- 退出内联编辑后再恢复执行
- `i` 进入多行 command modal
- modal 边框按钮的鼠标点击命中

这说明一个重要设计原则：

- `input.rs` 应该只负责“事件 -> 状态变化”
- 不直接负责绘制

### 6.3 `app/runner.rs`

[src/app/runner.rs](/Users/sean/Code/splitwatch/src/app/runner.rs:1)

职责：

- 周期调度
- 启动命令
- 回收命令结果
- 处理变量应用后的 rerun
- 退出时杀掉运行中的命令

这里近期有两个关键改动：

#### 1. 长运行状态

每个 tick 都会检查：

- 如果 pane 正在运行
- 并且已经运行时间超过 `interval_ms`
- 就把这个 pane 标记成 long-running

然后：

- 状态点变为 `theme.long_running`
- interval 文本变红

#### 2. 退出时可杀掉子进程

这是最近修掉的一个重要 bug。

之前的问题是：

- 后台线程 `wait()` 时占着 child 锁
- 退出时主线程也要拿这把锁 kill
- 结果退出卡住

现在的做法是：

- 等待线程独占真正的 `Child`
- `PaneState.child` 只保存 `ChildKiller`

这样退出时：

- UI 线程可以随时 kill
- 不会再被 `wait()` 的锁卡住

这是很典型的 Rust 并发资源建模案例：

- 与其让多个线程共享一个“完整对象”
- 不如只共享它真正需要暴露的能力

---

## 7. TUI 层设计

### 7.1 `tui.rs`

[src/tui.rs](/Users/sean/Code/splitwatch/src/tui.rs:1)

职责：

- 初始化终端
- 进入 raw mode
- 进入 alternate screen
- 开启鼠标
- 主事件循环
- 调用 `draw()`
- 退出时恢复终端

这里还包含最近新增的全局小窗口保护：

- 当终端太小，比如宽度/高度不足时
- 不再继续正常渲染 dashboard 和 modal
- 直接显示一条 resize 提示

这是为了避免宽度过小时出现 `Rect` 或 cursor 越界崩溃。

### 7.2 `tui/dashboard.rs`

[src/tui/dashboard.rs](/Users/sean/Code/splitwatch/src/tui/dashboard.rs:1)

职责：

- 绘制 pane 网格
- 调用 `PaneWidget`
- 管理 inline command overlay

最近的重要变化：

- 内联编辑不再只限空 pane
- 任意 pane 按 `Enter` 都可以进入 `>` 模式
- 进入内联编辑时，整个 pane 内容区会先 `Clear`
- 旧输出不会残留在背后

这就是典型的“overlay 渲染层”设计。

### 7.3 `tui/widgets.rs`

[src/tui/widgets.rs](/Users/sean/Code/splitwatch/src/tui/widgets.rs:1)

这里是复用 UI 组件层。

已经有这些控件：

- `SingleLineInput`
- `TextAreaInput`
- `InlineCommandInput`
- `ModalFrame`
- `StatusBarWidget`
- `PaneWidget`

这是项目组件化的核心。

其中：

#### `PaneWidget`

负责：

- pane 边框
- header
- 状态点
- 命令摘要
- output 内容

#### `InlineCommandInput`

负责：

- `>`
- `Type the command`
- 真正的命令内容
- 光标位置

#### `StatusBarWidget`

负责：

- 模式标签
- toast
- 模式相关按键提示

#### 当前 header 行为

pane 状态点现在是：

- `paused` -> `warning`
- `running` -> `accent`
- `long_running` -> `long_running`
- `success/failure` -> `success/error`

右上角 interval 控件现在还支持：

- long run 触发后变红
- 修改 interval 且超过上次卡顿时长时恢复

### 7.4 `tui/modals.rs`

[src/tui/modals.rs](/Users/sean/Code/splitwatch/src/tui/modals.rs:1)

职责：

- command modal
- rename modal
- save modal
- vars modal
- delete confirm
- help

这个文件之前重复非常多，现在已经开始收口。

### 7.5 `tui/actions.rs`

[src/tui/actions.rs](/Users/sean/Code/splitwatch/src/tui/actions.rs:1)

这是最近重构出来的统一动作区组件。

它抽象的是：

- 右下角边框上的动作文案
- 比如：
  - `Cancel · Confirm`
  - `Cancel · Delete`
  - `Quit`

它同时负责：

- 渲染
- group rect 计算
- 单个 action rect 计算
- 鼠标点击命中

这是当前最好的组件化重构成果之一。

### 7.6 `tui/ansi.rs`

[src/tui/ansi.rs](/Users/sean/Code/splitwatch/src/tui/ansi.rs:1)

职责：

- 解析 ANSI/SGR 样式
- 保留常见前景色、背景色、bold、underline、italic 等
- 过滤非渲染型控制序列

之所以要有它，是因为 pane 输出不是简单纯文本。

### 7.7 `tui/helpers.rs`

[src/tui/helpers.rs](/Users/sean/Code/splitwatch/src/tui/helpers.rs:1)

这是纯辅助逻辑层，放了：

- centered rect
- grid rect
- cursor 可见性判断
- 文本裁切
- 多行输入光标计算
- 小窗口保护常量

这类代码不应该混进 widgets 里，因为它们更偏“几何与文本计算”。

---

## 8. 命令执行模型

这是整个项目最值得学习的一块。

### 8.1 为什么不用 `std::process::Command` 直接 pipe

因为很多命令会根据：

- stdout 是否是 tty
- TERM 是什么

来决定是否输出：

- 颜色
- 图标
- 更完整的格式

例如：

- `eza --icons`
- `git status`
- `ls --color=auto`

所以这里改成了 PTY 执行。

### 8.2 当前模型

每个 pane 在 tick 时：

1. 如果正在运行，直接跳过
2. 如果没有命令，跳过
3. 如果是 paused，跳过
4. 如果到达 interval 或 `pending_run_once` 为真，就启动一次命令

也就是说：

- 同一 pane 永远不会有两个实例重叠
- 上一个命令不结束，下一个不会启动

### 8.3 PTY 执行链路

在 `spawn_command()` 里：

1. 创建 PTY
2. 用 shell `-lc` 执行命令
3. 设置终端相关环境变量
4. 启动 reader 线程读 PTY 输出
5. 当前线程等待 child 退出
6. 把输出和 exit code 发回主线程

这个模型兼顾了：

- 终端兼容性
- 简单实现
- 不需要异步 runtime

---

## 9. 近期功能演进总结

### 9.1 更完整的 pane 内联编辑

现在：

- 任意 pane 都能按 `Enter` 进入 `>` 单行编辑
- 有命令的 pane 会先暂停
- 退出后恢复执行
- 编辑态会清空整个 pane 内容区

### 9.2 更统一的 modal 边框动作区

现在 command/delete/help 的右下角动作按钮都统一成：

- 边框上的文字
- 支持鼠标点击
- 中间用 `·`
- 两侧留白不画边框线

### 9.3 更稳定的小窗口保护

终端过小时不再崩溃，而是显示 resize 提示。

### 9.4 长运行命令可视化

当某个命令运行超过自身 interval：

- 状态点变为 `theme.long_running`
- interval 数值变红

并且：

- interval 红色不会立刻自动恢复
- 用户只有把 interval 调大到超过最近一次卡顿时长，才会恢复

### 9.5 退出时不再被运行中的命令卡住

通过 `ChildKiller` 重构解决了退出卡死问题。

---

## 10. 这个项目值得学习的 Rust 点

### 10.1 明确区分“状态”和“渲染”

- `app/*` = 状态、输入、调度
- `tui/*` = 视觉和交互表现

这是非常值得模仿的分层。

### 10.2 先建模，再渲染

例如：

- `run_started_at`
- `long_running_latched`
- `last_long_running_ms`

这些都不是 UI hack，而是先在状态里建模，再让 UI 去消费。

### 10.3 尽量把纯逻辑抽出去

例如：

- `layout.rs`
- `vars.rs`
- `helpers.rs`

这让测试更容易写，也让主流程更清晰。

### 10.4 能力分离优于大锁共享

`Child` 与 `ChildKiller` 的拆分，就是典型例子。

### 10.5 组件化不一定依赖外部 UI 框架

虽然 `ratatui` 没有现成完整表单系统，但依然可以通过：

- `Widget`
- `StatefulWidget`
- 小组件模块

逐步搭出自己的组件层。

---

## 11. 当前可以继续怎么重构

现在已经抽了 `actions.rs`，下一步最值得做的是：

### 11.1 抽 `ModalShell`

统一：

- 圆角边框
- 标题
- inner rect
- 小窗口保护

### 11.2 抽 `FormField`

统一：

- 单行输入
- 多行输入
- 只读预览块
- 光标与 title 的组合

### 11.3 抽 `PaneHeader`

统一：

- 状态点
- title
- command summary
- interval 控件

这样以后做响应式 header 会更容易。

### 11.4 给关键行为补测试

目前测试更多是纯逻辑层。

后面可以补：

- `long_running_latched` 的行为测试
- interval 恢复颜色条件测试
- inline edit 的 pause/resume 行为测试

---

## 12. 如何本地运行和验证

### 12.1 构建

```bash
cargo build
cargo build --release
```

### 12.2 测试

```bash
cargo test
```

### 12.3 运行

```bash
cargo run --bin swatch -- -n 4
```

加载示例 preset：

```bash
cargo run --bin swatch -- -f ./examples/gpu-dev.toml
```

加载变量示例：

```bash
cargo run --bin swatch -- -f ./examples/train-debug.toml
```

### 12.4 推荐手工验证点

- 空 pane 按 `Enter`
- 有命令 pane 按 `Enter`
- `i` 多行弹窗
- delete modal 鼠标点击
- help modal `Quit`
- 长命令例如 `du -sh .`
- 运行中按 `q`
- 缩小终端窗口

---

## 13. 总结

这个项目非常适合作为一个 Rust 中等规模 TUI 项目的教学样本，因为它同时覆盖了：

- CLI 解析
- 状态机建模
- 配置文件读写
- TUI 组件化
- 终端事件处理
- PTY 子进程管理
- 长运行状态追踪
- 退出资源清理

它不是一个“纯界面 demo”，而是一个已经包含真实工程问题的小型应用。

从学习角度，最值得关注的是：

1. 状态和渲染分层
2. PTY 与进程管理
3. 组件化重构方式
4. 通过显式状态建模解决交互复杂度

如果后续继续演进，这个项目完全可以成为一个很好的 Rust TUI 工程模板。
