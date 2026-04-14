# sqtop

[English](./README.md) | [中文](./README.zh-CN.md)

![Rust](https://img.shields.io/badge/Rust-stable-orange?logo=rust)
![TUI](https://img.shields.io/badge/TUI-ratatui%20%2B%20crossterm-4c8eda)
![Platform](https://img.shields.io/badge/Platform-Linux%20%2F%20Slurm-2f855a)
![License](https://img.shields.io/badge/License-MIT-6b7280)

`sqtop` 是一个面向 Slurm 集群的交互式终端监控工具。它帮助普通 HPC 用户在不依赖 `slurmrestd`、不需要 root 权限、也不要求 Slurm JSON 输出的前提下，持续查看分区、队列、用户、节点和任务状态。

项目此前以 `s-top` 名义发布；当前可执行文件、crate 和 conda 包名称均为 `sqtop`。

## 为什么使用 sqtop

`squeue`、`sinfo` 和 `watch` 本身很可靠，但在下面这些问题上并不高效：

- 当前哪些分区最紧张？
- 我的任务和其他用户的任务分别占了多少？
- 现在是谁占用了最多任务和资源？
- 某个任务为什么还在 pending，或者被放到了哪里？
- 最近几次刷新里，队列压力是在上升还是下降？

`sqtop` 的目标就是让这些问题在终端里更快看清。

## 亮点

- 分区总览支持压力、归属拆分、运行/排队数量和趋势图
- 提供 `My Jobs`、`All Jobs`、`Users`、`Partition Detail`、`Node Detail` 多级视图
- 任务详情弹层采用结构化展示，而不是长文本堆叠
- `scancel` 相关流程默认先预览，再执行，避免误操作
- 基于文本型 Slurm 命令实现，兼容不支持 `--json` 的环境
- 支持搜索、过滤、排序、翻页、水平滚动和鼠标交互

## 截图

| 页面 | 预览 |
| --- | --- |
| Overview | ![Overview](docs/screenshots/overview-hero.png) |
| My Jobs | ![My Jobs](docs/screenshots/my-jobs.png) |
| All Jobs | ![All Jobs](docs/screenshots/all-jobs.png) |
| Users | ![Users](docs/screenshots/users.png) |
| Partition Detail | ![Partition Detail](docs/screenshots/partition-detail.png) |
| Node Detail | ![Node Detail](docs/screenshots/node-detail.png) |
| Job Detail | ![Job Detail](docs/screenshots/job-detail.png) |
| Cancel Preview | ![Cancel Preview](docs/screenshots/cancel-preview.png) |

## 安装

### 从 crates.io 安装

```bash
cargo install sqtop
```

### 通过 conda 安装

```bash
conda install -c wubeizhongxinghua sqtop
```

如果希望后续可以直接执行 `conda install sqtop`，先把 channel 加入本地配置：

```bash
conda config --add channels wubeizhongxinghua
conda install sqtop
```

### 从源码构建

```bash
cargo build --release
./target/release/sqtop
```

## 快速开始

启动 TUI：

```bash
sqtop
```

只采集一次并输出摘要：

```bash
sqtop --once
```

调整刷新间隔：

```bash
sqtop --interval 2
```

输出原始与解析后的采集数据用于排查：

```bash
sqtop --debug-dump
```

## 典型使用场景

- 提交任务前先看分区是否拥堵
- 对比自己和其他用户当前的队列占用
- 进入某个分区或节点排查任务放置情况
- 按 pending reason、placement、owner 过滤任务
- 在取消任务前先检查目标任务是否真的应该取消

## 页面说明

### Overview

默认首页，展示分区压力、Mine / Others 拆分、运行/排队统计以及滚动趋势。

### My Jobs

聚焦当前用户的活跃任务，适合日常查看和单任务操作。

### All Jobs

展示全局活跃队列，并高亮当前用户的任务。

### Users

按用户展示运行任务数、排队任务数、总任务数和资源足迹；下半部分显示所选用户的活跃任务。

### Partition Detail

进入单个分区后，可查看分区趋势、节点状态分布、节点列表以及该分区下的任务。

### Node Detail

进入单个节点后，可按 `user`、`state`、`where`、`why` 进行交互式筛选。

### Job Detail

以结构化弹层展示单个任务的身份、资源、调度、放置和执行路径信息。

### Cancel Preview / Result

在真正执行 `scancel` 前先展示允许取消与禁止取消的任务集合，并在执行后给出逐项结果。

## 快捷键与鼠标

### 键盘

| 按键 | 作用 | 范围 |
| --- | --- | --- |
| `Tab` / `Shift-Tab` | 切换顶层页面 | 全局 |
| `q` | 从详情页返回；在顶层页退出 | 全局 |
| `j` / `k` / 上下方向键 | 移动选择或滚动 | 列表与弹层 |
| `Space` / `b` | 下一页 / 上一页 | 列表与弹层 |
| `g` / `G` | 跳到顶部 / 底部 | 列表与弹层 |
| `Enter` | 打开当前选中项详情 | Overview 与列表 |
| `/` | 开始实时搜索 | 可搜索页面 |
| `s` | 切换排序字段 | Overview、Users、任务列表 |
| `f` | 切换队列状态过滤 | 任务列表 |
| `m` | 切换 mine-only | 共享页面 |
| `p` | 固定或取消固定分区 | Overview 与任务页 |
| `[` / `]` | 切换选中节点 | Partition Detail |
| `n` | 打开选中节点 | Partition Detail |
| `u` / `w` / `y` / `c` | 修改或清空节点过滤 | Node Detail |
| `i` | 打开任务详情 | 任务列表 |
| `x` | 取消当前选中任务 | 任务列表 |
| `X` | 预览批量取消 | 任务列表 |
| `Left` / `Right` | 水平移动宽表格视野 | 宽表格 |

### 鼠标

| 操作 | 结果 |
| --- | --- |
| 点击标签页 | 切换页面 |
| 点击某一行 | 选中该行 |
| 双击某一行 | 打开详情 |
| 点击可排序表头 | 按该列排序 |
| 滚轮 | 滚动当前列表或弹层 |
| 点击弹层按钮 | 触发对应动作 |
| 点击任务详情弹层外部 | 关闭弹层 |

## 命令行参数

| 参数 | 说明 |
| --- | --- |
| `--interval <seconds>` | 刷新间隔，默认 `2.0` |
| `--user <name>` | 覆盖 Mine / Others 使用的当前用户身份 |
| `--all` | 启动时进入 `All Jobs` 页面 |
| `--no-all-jobs` | 禁用 `All Jobs` 页面 |
| `--theme <auto\|dark\|light>` | 选择主题 |
| `--advanced-resources` | 强制显示高级资源列 |
| `--no-advanced-resources` | 隐藏高级资源列 |
| `--debug-dump` | 输出原始与解析后的数据后退出 |
| `--once` | 单次采集并输出摘要后退出 |
| `--compact` | 使用更紧凑的布局 |
| `--no-color` | 关闭颜色输出 |

## 数据来源与兼容性

主界面实时采集基于以下文本型 Slurm 命令：

- `sinfo`
- `squeue`
- `scontrol show partition`
- `scontrol show node`
- `scontrol show job`

`sacct` 仅用于详情补充或可选历史视图，不是主界面运行的必需条件。

采集设计遵循以下原则：

- 使用显式字段分隔符，而不是依赖空格猜列
- 所有外部命令都带超时控制
- 某个命令失败时只降级对应面板，不导致整个 TUI 崩溃
- 可选字段在模型层保持可选，不强行假设站点一定提供

## 项目状态

仓库当前通过 GitHub tag 持续发布版本。后续改进建议以 issue 列表和 release 记录为准，而不是在 README 中维护一份容易过时的长期路线图。

## 项目结构

| 路径 | 说明 |
| --- | --- |
| `src/collector/` | Slurm 命令执行、超时、取消、原始数据采集 |
| `src/model/` | 解析器、标准化数据结构、聚合逻辑 |
| `src/app.rs` | 应用状态、刷新调度、过滤/排序、事件处理 |
| `src/ui/` | 视图渲染、主题、组件、鼠标命中逻辑 |
| `src/cli.rs` | 命令行参数与当前用户识别 |
| `src/config.rs` | 可选配置支持 |
| `recipe/` | conda recipe 与构建脚本 |
| `.github/workflows/` | Release 打包与注册表发布自动化 |

## 限制与注意事项

- `ReqTRES`、`AllocTRES`、`GRES`、内存、GPU 等字段是否可用取决于站点配置
- 对于可投递到多个分区的 pending 任务，分区级 pending 聚合可能重复计数
- 终端较窄时，宽表格仍需通过水平移动查看全部列
- 趋势图依赖终端字体对 Unicode 符号的支持
- 当前 conda 打包流程优先覆盖 Linux `x86_64`
- 当前 Linux conda 包的 `glibc` 兼容基线设为 `2.17`

## 社区与支持

- [贡献指南](CONTRIBUTING.md)
- [行为准则](CODE_OF_CONDUCT.md)
- [支持与使用问题](SUPPORT.md)
- [安全策略](SECURITY.md)
- [引用方式](CITATION.cff)
- [变更日志](CHANGELOG.md)

## 许可证

本项目采用 MIT 许可证，详见 [LICENSE](LICENSE)。
