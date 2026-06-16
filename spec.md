# 轻量级Linux程序管理面板 (MiniPanel) Spec

## Why

用户需要一个类似1Panel/宝塔的轻量级程序管理面板，但无需Docker支持，专注于自定义程序和脚本管理，且能在OpenWrt和Alpine Linux等低配置环境中运行，内存占用不超过50MB。

## What Changes

- 新增go后端服务，提供RESTful API管理程序/脚本生命周期
- 新增轻量级Web前端，纯HTML/CSS/JavaScript实现，无大型前端框架依赖
- 新增进程监控模块，基于/proc和sysfs采集系统与进程资源数据
- 新增日志管理模块，支持实时日志流和关键词搜索
- 新增系统信息展示模块
- 新增身份验证与HTTPS支持
- 新增OpenWrt和Alpine Linux的构建脚本与服务配置
- **BREAKING**: 无（全新项目）

## Impact

- Affected specs: 全系统为新开发，无既有规格影响
- Affected code: 整个minipanel代码库

## ADDED Requirements

### Requirement: 程序/脚本管理

The system SHALL提供对自定义程序和脚本的完整生命周期管理。

#### Scenario: 添加程序

- **WHEN** 用户通过Web界面或API提交程序信息（名称、启动命令、工作目录、启动参数、环境变量）
- **THEN** 系统持久化该程序配置，并可在列表中查看

#### Scenario: 启动程序

- **WHEN** 用户请求启动指定程序
- **THEN** 系统以子进程方式启动该程序，并记录其PID

#### Scenario: 停止程序

- **WHEN** 用户请求停止指定程序
- **THEN** 系统向该程序进程发送终止信号，并等待其退出

#### Scenario: 重启程序

- **WHEN** 用户请求重启指定程序
- **THEN** 系统先停止再启动该程序

#### Scenario: 查看程序状态

- **WHEN** 用户查看程序列表
- **THEN** 系统显示每个程序的运行状态（运行中/已停止/异常）、PID、运行时长

### Requirement: 进程监控

The system SHALL实时展示系统及被管理进程的资源使用情况。

#### Scenario: 系统资源概览

- **WHEN** 用户打开监控页面
- **THEN** 系统显示整体CPU使用率、内存使用率、磁盘I/O速率

#### Scenario: 进程资源详情

- **WHEN** 用户查看某个运行中程序
- **THEN** 系统显示该进程的CPU、内存、磁盘I/O占用

### Requirement: 日志查看

The system SHALL提供程序运行日志的查看与搜索功能。

#### Scenario: 实时日志流

- **WHEN** 用户打开某个程序的日志页面
- **THEN** 系统通过WebSocket推送该程序的最新日志输出

#### Scenario: 日志搜索

- **WHEN** 用户在日志页面输入关键词并搜索
- **THEN** 系统返回包含该关键词的最近日志行（限制数量以避免内存溢出）

### Requirement: 配置管理

The system SHALL允许用户为每个程序/脚本设置启动参数和环境变量。

#### Scenario: 编辑程序配置

- **WHEN** 用户编辑程序配置（启动命令、工作目录、启动参数、环境变量、自动启动选项）
- **THEN** 系统保存新配置，并在下次启动时生效；若程序正在运行，提示需重启

### Requirement: 系统信息展示

The system SHALL展示基本系统信息。

#### Scenario: 查看系统信息

- **WHEN** 用户访问系统信息页面
- **THEN** 系统显示主机名、操作系统、架构、运行时间、总内存、磁盘空间等

### Requirement: 身份验证

The system SHALL实现基本的身份验证机制。

#### Scenario: 登录

- **WHEN** 用户访问管理界面
- **THEN** 系统要求输入用户名和密码；验证通过后方可访问

#### Scenario: 会话管理

- **THEN** 系统使用JWT或Session管理登录状态，设置合理的过期时间

### Requirement: HTTPS通信

The system SHALL支持通过HTTPS提供Web服务。

#### Scenario: HTTPS服务

- **WHEN** 系统启动时检测到有效的证书和私钥文件
- **THEN** 系统通过HTTPS提供服务；否则回退到HTTP并记录警告

### Requirement: 平台适配与构建

The system SHALL支持在OpenWrt和Alpine Linux上静态编译运行。

#### Scenario: OpenWrt构建

- **THEN** 提供针对OpenWrt常见架构（armv7, aarch64, mips, mipsel, x86\_64）的交叉编译脚本
- **THEN** 生成的可执行文件为静态链接，不依赖外部动态库

#### Scenario: Alpine Linux构建

- **THEN** 提供针对Alpine Linux常见架构（x86\_64, aarch64, armv7）的交叉编译脚本
- **THEN** 生成的可执行文件为静态链接，不依赖外部动态库（特别是musl兼容）

### Requirement: 部署与服务化

The system SHALL提供简单的部署方式并支持作为系统服务运行。

#### Scenario: 部署脚本

- **THEN** 提供一键安装/卸载脚本，自动配置可执行文件路径、数据目录、服务配置

#### Scenario: 系统服务

- **THEN** 在支持systemd的系统上，提供systemd service文件
- **THEN** 在OpenWrt系统上，提供procd init脚本

### Requirement: 自我保护

The system SHALL具备基本的自我保护机制，防止误操作导致面板不可用。

#### Scenario: 防止停止面板自身

- **WHEN** 用户尝试停止或重启面板自身的进程
- **THEN** 系统拒绝该操作并给出明确提示

#### Scenario: 数据目录保护

- **WHEN** 用户配置程序工作目录时
- **THEN** 系统阻止将工作目录设置为面板自身的核心数据目录或系统关键目录

### Requirement: 内存优化

The system SHALL在低配置设备上运行时，整体内存占用不超过50MB。

#### Scenario: 空闲状态

- **WHEN** 系统处于空闲状态（无活跃WebSocket连接，无大量日志处理）
- **THEN** 进程RSS内存占用不超过50MB

#### Scenario: 活跃状态

- **WHEN** 系统处于正常使用状态（少量并发用户，常规监控数据采集）
- **THEN** 进程RSS内存占用应尽量控制在50MB以内，允许短暂峰值不超过80MB

