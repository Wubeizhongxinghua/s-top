# s-top

[English](./README.md) | [中文](./README.zh-CN.md)

> 面向 Slurm 集群的现代化终端监控面板，用来快速看懂分区压力、任务归属、用户占用和任务详情。

![Rust](https://img.shields.io/badge/Rust-stable-orange?logo=rust)
![TUI](https://img.shields.io/badge/TUI-ratatui%20%2B%20crossterm-4c8eda)
![Platform](https://img.shields.io/badge/Platform-Linux%20%2F%20Slurm-2f855a)
![Mode](https://img.shields.io/badge/Data%20Path-text%20Slurm%20commands-805ad5)
![Status](https://img.shields.io/badge/Status-active%20iteration-0ea5e9)

## 项目简介

`s-top` 是一个全屏 Slurm TUI 工具，重点不是节点级硬件遥测，而是**调度视角**：

- 哪些 partition 正在紧张
- 我占了多少，别人占了多少
- 谁在消耗资源
- 哪些 job 正在 running 或 pending
- 队列状态最近是上升还是下降

它默认面向普通 HPC 用户环境：

- 不需要 root
- 不依赖 `slurmrestd`
- 不把 `squeue --json` / `sinfo --json` 当主路径
- 能在字段缺失、权限有限、环境不一致时优雅降级

![s-top 总览主图](docs/screenshots/overview-hero.png)

## 为什么做这个工具

Slurm 原生命令很强，但在“快速看懂全局状态”这件事上不够顺手：

- `squeue` 很准，但不够直观
- `sinfo` 能看节点状态，但不容易串起 ownership、压力和队列状态
- `watch` 只能刷新，不能提供结构化交互

`s-top` 想解决这些真实痛点：

- **总览不直观**：一眼看不出哪个分区最紧张
- **归属不直观**：很难快速判断 Mine / Others
- **用户维度缺失**：谁在占资源、主要占了哪些分区，不够容易看
- **任务详情不够好读**：纯文本 detail 很难扫读
- **趋势不可见**：只看当前值，看不到最近变化

## 亮点

### 总览与可视化

- **Overview 首页**：pressure、Mine / Others、Running / Pending、趋势图同屏展示
- **稳定的 partition 颜色**：同一个 partition 在多个页面尽量保持同色
- **滚动趋势图**：Overview、My Jobs、All Jobs、Partition Detail 都能看到 running / pending 的变化趋势

### 队列与排障

- **My Jobs / All Jobs**：支持搜索、过滤、排序、水平视野移动
- **User View**：从用户角度看 jobs、running/pending、资源占用和主要分区
- **Partition Detail / Node Detail**：按分区和节点继续下钻

### Job 操作与详情

- **结构化 Job Detail**：按 Basic、Resources、Scheduling、Placement / Reason、Paths / Extra 分组展示
- **安全的取消流程**：单个和批量 `scancel` 都有预览和确认
- **点击外部关闭 detail**：鼠标路径更直观

### 交互体验

- **实时搜索**：输入即过滤
- **点击表头排序**：支持鼠标排序
- **左右方向键移动水平视野**：宽表格可完整查看 Name、Resource footprint 等宽列
- **快速退出**：后台命令支持取消，不会因为慢命令拖住退出

## 功能概览

| 模块 | 能力 |
| --- | --- |
| Overview | 分区压力、Mine/Other 拆分、running/pending 拆分、全局趋势图 |
| Jobs | 我的任务、全部任务、过滤、排序、资源 footprint 条形图、水平滚动 |
| Users | 用户级 running/pending/jobs/cpu/gpu 汇总，以及所选用户的活跃任务 |
| Partition | 分区压力、节点状态分布、分区趋势图、分区内 jobs |
| Node | user/state/where/why 交互式筛选 |
| Job Detail | 分组信息面板，字段对齐，支持点击外部关闭 |
| Mouse | 点 tab、点行、双击打开、点表头排序、点返回按钮、点弹层外关闭 |

## 预览说明

> 下面的截图路径已经直接写进 README。后续你只要把图片放到 `docs/screenshots/`，文档结构不用再改。

### Overview

- 看分区压力
- 看 Mine Running / Mine Pending / Others Running / Others Pending
- 看最近一段时间 running / pending 的变化

### My Jobs 页面

- 实时搜索、过滤、排序、水平视野移动
- 通过水平滚动完整查看 `Name`
- `resource footprint` 聚焦 Node / CPU / GPU

![My Jobs 页面](docs/screenshots/my-jobs.png)

### All Jobs 页面

- 看全局活跃队列
- 高亮自己的任务
- `Where / Why` 优先于 `Name`，更利于排障

![All Jobs 页面](docs/screenshots/all-jobs.png)

### Users 页面

- 看不同用户的 running / pending / total jobs
- 看用户资源 footprint 排名
- 看所选用户的活跃任务下钻

![Users 页面](docs/screenshots/users.png)

### Partition Detail 页面

- 看分区趋势图
- 看节点状态分布
- 看该分区下的 jobs 列表

![Partition Detail 页面](docs/screenshots/partition-detail.png)

### Node Detail 页面

- 针对单个节点看任务列表
- 支持 `user` / `state` / `where` / `why` 交互式筛选
- 适合节点级排障

![Node Detail 页面](docs/screenshots/node-detail.png)

### Jobs 页面

- 资源 footprint 统一为“左侧 bar，右侧数字”
- 资源 footprint 只显示 Node / CPU / GPU 三项
- `Where / Why` 在 `Name` 之前，更利于排障
- `Left` / `Right` 用来查看右侧宽列

### Job Detail

- 不再是 key-value 文本堆叠
- 改为结构化面板
- 点击面板外部即可关闭

![Job Detail 弹层](docs/screenshots/job-detail.png)

### Cancel Preview

- 单任务取消和批量取消都会先进入确认界面
- 能明确看到哪些任务会被取消、哪些不会

![取消确认弹层](docs/screenshots/cancel-preview.png)

### 可选 GIF / 动图位

如果后面想补动图，比较值得放这些场景：

- `docs/screenshots/demo-overview.gif`
- `docs/screenshots/demo-search-and-sort.gif`
- `docs/screenshots/demo-job-detail-and-cancel.gif`
- `docs/screenshots/demo-node-filtering.gif`

## 数据来源

主路径使用轻量文本命令：

- `sinfo -Nh -o '%P<sep>%t<sep>%N<sep>%c<sep>%m<sep>%G'`
- `squeue -h -t PENDING,RUNNING,CONFIGURING,COMPLETING,SUSPENDED -o '%i<sep>%u<sep>%a<sep>%P<sep>%j<sep>%T<sep>%M<sep>%l<sep>%D<sep>%C<sep>%b<sep>%V<sep>%Q<sep>%R'`
- `scontrol show partition`
- `scontrol show node -o <node>`
- `squeue -h -w <node> ...`
- `scontrol show job -o <jobid>`
- `sacct -n -P -X -j <jobid> ...`

设计原则：

- 文本输出优先，JSON 只是可选能力
- 用显式分隔符 `\x1f`，避免脆弱空格解析
- 所有外部命令都有超时和取消
- UI 层不直接阻塞等待 shell

## 安装

### 依赖

- Linux
- Rust stable
- `PATH` 中可用的 Slurm 客户端命令
- 支持全屏 TUI 的终端

### 从源码构建

```bash
cargo build --release
```

### 本地安装

```bash
cargo install --path .
```

### 下载预编译二进制

带版本号的 GitHub Release 会自动提供可直接下载的压缩包，目前覆盖：

- Linux `x86_64`
- macOS `x86_64`
- macOS `aarch64` / Apple Silicon
- Windows `x86_64`

每个压缩包都包含：

- `s-top` 可执行文件
- `README.md`
- `README.zh-CN.md`
- `config.example.toml`

### 运行

```bash
./target/release/s-top
```

修改刷新间隔：

```bash
cargo run --release -- --interval 2
```

单次采样摘要：

```bash
./target/release/s-top --once
```

调试数据导出：

```bash
./target/release/s-top --debug-dump
```

## 快速开始

1. 编译项目。
2. 运行 `./target/release/s-top`。
3. 首先看 **Overview**，判断哪些分区最紧张。
4. 用 `Tab` 切到 **My Jobs**、**Users** 或 **All Jobs**。
5. 用 `/` 搜索，用 `s` 排序，用 `f` 过滤，用 `Enter` 看 detail。
6. 宽表格用 `Left` / `Right` 横向移动，完整查看 `Name`、`Resource footprint` 等宽列。

## CLI 参数

| 参数 | 说明 |
| --- | --- |
| `--interval <seconds>` | 刷新间隔，默认 `2.0` |
| `--user <name>` | 覆盖 Mine / Others 使用的当前用户身份 |
| `--all` | 启动时直接进入 All Jobs |
| `--no-all-jobs` | 禁用 All Jobs 页面 |
| `--theme <auto|dark|light>` | 主题选择 |
| `--advanced-resources` | 强制显示高级资源列 |
| `--no-advanced-resources` | 隐藏高级资源列 |
| `--debug-dump` | 输出原始与解析后 JSON 后退出 |
| `--once` | 输出文本摘要后退出 |
| `--compact` | 使用更紧凑的布局 |
| `--no-color` | 关闭颜色 |

## 键位说明

| 键位 | 动作 | 适用页面 |
| --- | --- | --- |
| `q` | 退出 | 全局 |
| `Tab` / `Shift-Tab` | 切换顶层页面 | 全局 |
| `j` / `k` / 方向键上下 | 移动选择 | 列表页 |
| `Enter` | 打开详情 | Overview / Jobs |
| `b` / `Esc` | 返回 | 详情页 / 弹层 |
| `/` | 进入实时搜索 | 全局 |
| `s` | 循环切换排序字段 | Overview / Users / Jobs |
| `f` | 循环切换状态过滤 | Jobs |
| `m` | 切换 mine-only | 共享页面 |
| `g` | 切换 metric 模式 | Overview / Partition |
| `p` | pin / unpin 当前 partition | Overview / Jobs |
| `[` / `]` | 切换当前节点 | Partition Detail |
| `n` | 打开选中的节点 | Partition Detail |
| `u` | 切换 node user filter | Node Detail |
| `w` | 编辑 node where filter | Node Detail |
| `y` | 编辑 node why filter | Node Detail |
| `c` | 清空 node filters | Node Detail |
| `i` | 打开 job detail | Jobs |
| `x` | 取消当前 job | Jobs |
| `X` | 预览批量取消 | Jobs |
| `Left` / `Right` | 水平移动表格视野 | 宽 Jobs 表格 |

## 鼠标操作

| 操作 | 结果 |
| --- | --- |
| 点击顶部 tab | 切换页面 |
| 点击行 | 选中 |
| 双击行 | 打开详情 |
| 点击可排序表头 | 按该列排序 |
| 再次点击同一表头 | 反转排序方向 |
| 滚轮 | 上下滚动列表 |
| 点击 footer 按钮 | 执行动作 |
| 点击 job detail 外部区域 | 关闭 detail |
| 点击 `← Overview (b)` | 返回 Overview |

## 页面说明

### Overview

这是主监控页，重点回答：

- 现在哪个 partition 最紧张
- Mine 和 Others 各占多少
- Running / Pending 的分布如何
- 最近一段时间趋势如何变化

### My Jobs

只看当前用户的活跃任务，适合日常操作和排障。

### Users

按用户视角看当前资源占用，并在下半区查看所选用户的活跃 jobs。

### All Jobs

看全体活跃 jobs，并对当前用户的任务做高亮。

### Partition Detail

进入某个 partition 后，看该分区的：

- 压力
- ownership
- running / pending 状态
- 节点状态分布
- 趋势图
- 分区内任务和节点

### Node Detail

进入节点后，可按：

- user
- state
- where
- why

进行交互式过滤。

### Job Detail

结构化展示：

- Basic
- Resources
- Scheduling
- Placement / Reason
- Paths / Extra

## 项目结构

| 路径 | 作用 |
| --- | --- |
| `src/collector/` | Slurm 命令执行、超时、取消、缓存 |
| `src/model/` | 文本解析、标准化结构、partition/user 聚合 |
| `src/app.rs` | 全局状态、刷新、搜索/过滤/排序、事件处理 |
| `src/ui/` | 渲染、主题、组件、modal、鼠标命中、趋势图 |
| `src/cli.rs` | CLI 与当前用户识别 |
| `src/config.rs` | 配置文件支持 |
| `config.example.toml` | 示例配置 |

## 配置

可选配置文件：

```text
~/.config/s-top/config.toml
```

示例：

```toml
interval = 2.0
all_jobs_enabled = true
start_in_all_jobs = false
show_advanced_resources = true
compact = false
no_color = false
theme = "dark"
```

当前可配置项主要包括：

- 刷新间隔
- 当前用户身份覆盖
- 页面开关
- 主题
- 紧凑模式
- 颜色开关
- 是否显示高级资源列

## Notes / Troubleshooting

### 集群没有 JSON serializer

没关系，`s-top` 本来就是按文本 Slurm 命令设计的。

### 表格太宽

用 `Left` / `Right` 横向移动。`Where / Why` 已经提前，`Name` 放在更右侧，需要时再滚过去看完整内容。

### 某些 detail 字段缺失

这通常是 Slurm 站点配置差异导致。`s-top` 会显示 `N/A`，而不是崩溃。

### 趋势图字符显示不均匀

趋势图使用 Unicode dot 风格字符。大多数现代终端没问题，但非常老的终端字体可能对齐稍差。

## 已知限制

- `ReqTRES`、`AllocTRES`、`GRES`、memory、GPU 等字段取决于站点配置
- 多 partition 的 pending job 仍按可见 partition 归属计入各分区 pending 统计
- 终端非常窄时，需要依赖水平视野移动查看宽列
- dot 风格趋势图依赖终端字体质量
- history 相关代码仍保留在树中，但不在默认刷新主路径里
- 当前仓库还没有正式的 `LICENSE` 文件，发布到 GitHub 前建议补齐

## Roadmap

- 更细的 node-level 资源视图
- 更深入的 user detail 页面
- 继续把可复用 UI 组件拆到更清晰的子模块
- 补充截图 / GIF 资源，提升项目主页展示效果

## 贡献

欢迎提 issue 和 PR。

建议本地提交流程：

```bash
cargo fmt
cargo test
cargo build --release
```

当前项目方向比较明确：

- 首屏要快
- 刷新不能阻塞 UI
- 要兼容真实 Slurm 用户环境
- 输出必须可读，不能为了压缩信息而牺牲理解成本

## License

当前仓库尚未包含正式 `LICENSE` 文件。对外发布前，请补充你希望采用的开源协议。
