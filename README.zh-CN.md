# s-top

[English](./README.md) | [中文](./README.zh-CN.md)

![Rust](https://img.shields.io/badge/Rust-stable-orange?logo=rust)
![TUI](https://img.shields.io/badge/TUI-ratatui%20%2B%20crossterm-4c8eda)
![Platform](https://img.shields.io/badge/Platform-Linux%20%2F%20Slurm-2f855a)
![Data Path](https://img.shields.io/badge/Data%20Path-text%20Slurm%20commands-805ad5)

`s-top` 是一个面向 Slurm 集群的终端监控工具。它关注的是调度与队列视角，而不是底层硬件遥测。项目设计目标是在普通 HPC 用户环境中，提供持续刷新、结构清晰、可交互的集群状态视图。

![Overview](docs/screenshots/overview-hero.png)

## 项目简介

`s-top` 主要回答以下问题：

- 哪些分区当前最紧张
- 当前用户与其他用户分别占用了多少资源
- 哪些任务处于运行或排队状态
- 哪些用户占用了最多资源
- 队列压力在最近一段时间内如何变化

项目默认面向常见的普通用户环境：

- 不要求 root 权限
- 不依赖 `slurmrestd`
- 不要求 `squeue --json` 或 `sinfo --json`
- 当站点未提供某些可选字段时可以优雅降级

## 主要功能

- 基于 `ratatui` 和 `crossterm` 的全屏终端界面
- 周期性刷新，外部命令带超时与取消控制
- 分区总览：压力、归属、运行/排队分布、趋势图
- `My Jobs` 与 `All Jobs`：搜索、过滤、排序、水平浏览
- `Users`：用户级任务与资源汇总
- `Partition Detail` 与 `Node Detail` 下钻视图
- 结构化 `Job Detail` 弹层
- 保守的单任务/批量 `scancel` 流程
- 鼠标支持：分页、表头排序、行选择、弹层操作

## 截图

README 已直接引用最终截图路径。后续只需把图片放入 `docs/screenshots/`，无需再调整文档结构。

### Overview

![Overview](docs/screenshots/overview-hero.png)

### My Jobs

![My Jobs](docs/screenshots/my-jobs.png)

### All Jobs

![All Jobs](docs/screenshots/all-jobs.png)

### Users

![Users](docs/screenshots/users.png)

### Partition Detail

![Partition Detail](docs/screenshots/partition-detail.png)

### Node Detail

![Node Detail](docs/screenshots/node-detail.png)

### Job Detail

![Job Detail](docs/screenshots/job-detail.png)

### Cancel Preview

![Cancel Preview](docs/screenshots/cancel-preview.png)


## 页面说明

### Overview

默认首页，展示分区压力、归属拆分、运行/排队统计以及全局趋势。

### My Jobs

展示当前用户的活跃任务，适合日常查看与任务操作。

### All Jobs

展示全局活跃队列，并对当前用户的任务做高亮区分。

### Users

展示用户级别的运行任务数、排队任务数、总任务数、资源占用以及主要分区；下方区域显示所选用户的活跃任务。

### Partition Detail

展示单个分区的详细视图，包括趋势、节点状态分布、节点列表以及该分区下的任务。

### Node Detail

展示单个节点上的任务列表，并支持 `user`、`state`、`where`、`why` 交互式筛选。

### Job Detail

以结构化弹层展示任务详情，按字段用途分组，而不是输出难以阅读的纯文本块。

## 安装

### 环境要求

- Linux
- Rust stable 工具链
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

### 从 crates.io 安装

```bash
cargo install s-top
```

### 通过 conda 安装

在 conda 包发布到项目 channel 后，可以直接执行：

```bash
conda install -c wubeizhongxinghua s-top
```

## 使用方法

### 启动

```bash
./target/release/s-top
```

### 常用方式

```bash
./target/release/s-top --interval 2
./target/release/s-top --once
./target/release/s-top --debug-dump
```

### 命令行参数

| 参数 | 说明 |
| --- | --- |
| `--interval <seconds>` | 刷新间隔，默认 `2.0` |
| `--user <name>` | 覆盖 Mine / Others 使用的当前用户身份 |
| `--all` | 启动时进入 `All Jobs` |
| `--no-all-jobs` | 禁用 `All Jobs` 页面 |
| `--theme <auto\|dark\|light>` | 选择主题 |
| `--advanced-resources` | 强制显示高级资源列 |
| `--no-advanced-resources` | 隐藏高级资源列 |
| `--debug-dump` | 输出原始与解析后的数据后退出 |
| `--once` | 单次采集并输出摘要后退出 |
| `--compact` | 使用更紧凑的布局 |
| `--no-color` | 关闭颜色输出 |

## 快捷键与交互

### 键盘

| 按键 | 作用 | 范围 |
| --- | --- | --- |
| `q` | 退出 | 全局 |
| `Tab` / `Shift-Tab` | 切换顶层页面 | 全局 |
| `j` / `k` / 上下方向键 | 移动选择 | 列表页 |
| `Enter` | 打开详情 | Overview 与任务列表 |
| `b` / `Esc` | 返回或关闭弹层 | 详情页与弹层 |
| `/` | 进入实时搜索 | 全局 |
| `s` | 循环切换排序字段 | Overview、Users、任务列表 |
| `f` | 循环切换队列状态过滤 | 任务列表 |
| `m` | 切换 mine-only 模式 | 共享页面 |
| `g` | 切换统计指标模式 | Overview 与 Partition Detail |
| `p` | 固定或取消固定当前分区 | Overview 与任务页 |
| `[` / `]` | 切换选中节点 | Partition Detail |
| `n` | 打开选中节点 | Partition Detail |
| `u` | 切换节点用户过滤 | Node Detail |
| `w` | 编辑节点 `where` 过滤 | Node Detail |
| `y` | 编辑节点 `why` 过滤 | Node Detail |
| `c` | 清空节点过滤条件 | Node Detail |
| `i` | 打开任务详情 | 任务列表 |
| `x` | 取消当前选中任务 | 任务列表 |
| `X` | 预览批量取消 | 任务列表 |
| `Left` / `Right` | 水平移动列视野 | 宽表格 |

### 鼠标

| 操作 | 结果 |
| --- | --- |
| 点击分页标签 | 切换页面 |
| 点击某一行 | 选中该行 |
| 双击某一行 | 打开详情 |
| 点击可排序表头 | 按该列排序 |
| 再次点击同一表头 | 反转排序方向 |
| 滚轮 | 滚动当前列表 |
| 点击底部操作项 | 触发对应动作 |
| 点击任务详情弹层外部 | 关闭弹层 |

## 数据来源与兼容性

主路径使用 Slurm 文本命令：

- `sinfo`
- `squeue`
- `scontrol show partition`
- `scontrol show node`
- `scontrol show job`

`sacct` 仅用于必要的历史或详情补充，不是主界面实时刷新的前提条件。

数据采集遵循以下原则：

- 使用显式字段分隔符，而不是基于空格猜测列边界
- 所有外部命令都带超时控制
- 某个命令失败时，仅降级对应面板，不使整个界面崩溃
- 可选字段在模型层保持可选，不强行假定站点一定提供

## 项目结构

| 路径 | 说明 |
| --- | --- |
| `src/collector/` | Slurm 命令执行、超时处理、取消控制、原始数据采集 |
| `src/model/` | 解析器、标准化数据结构、聚合逻辑 |
| `src/app.rs` | 应用状态、刷新调度、过滤/排序、事件处理 |
| `src/ui/` | 视图渲染、主题、组件、鼠标命中逻辑 |
| `src/cli.rs` | 命令行参数与当前用户识别 |
| `src/config.rs` | 可选配置支持 |
| `recipe/` | conda recipe 与构建脚本 |
| `.github/workflows/` | Release 打包与注册表发布自动化 |
| `config.example.toml` | 示例配置文件 |

## 限制与注意事项

- `ReqTRES`、`AllocTRES`、`GRES`、内存、GPU 等字段是否可用取决于站点配置
- 对于可投递到多个分区的 pending 任务，分区级 pending 聚合可能出现重复计数
- 终端宽度较小时，宽表格仍需通过水平移动查看全部列
- 趋势图依赖终端字体对 Unicode 符号的支持
- 当前 conda 打包流程首先覆盖 Linux `x86_64`；如有需要，可以继续扩展到其他 conda 平台
- 当前 conda 包的 Linux `glibc` 兼容基线设为 `2.17`

## License

本项目采用 MIT 许可证，详见 [LICENSE](LICENSE)。
