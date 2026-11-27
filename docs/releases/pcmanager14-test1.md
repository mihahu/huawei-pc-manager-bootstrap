# 电脑管家 14 适配测试版 pcmanager14-test1

发布日期：2026-11-27。维护与发布：[mihahu](https://github.com/mihahu)。

## 使用反馈

用户已安装电脑管家 `14.0.7.310(C233)`，并反馈星芒 O1 Pro 已成功连接、目前测试正常。手机系统此前由用户描述为鸿蒙 7，完整系统构建号和连接方式尚未单独记录。该结果仅代表本机组合，不保证全部设备或每项协同功能均兼容。

## 下载与使用

- [Windows 工具包](https://github.com/mihahu/huawei-pc-manager-bootstrap/releases/download/pcmanager14-test1/huawei-pc-manager-bootstrap-pcmanager14-test1-windows.zip)：启动器、配套核心 DLL、说明和构建信息。
- [配套源码](https://github.com/mihahu/huawei-pc-manager-bootstrap/releases/download/pcmanager14-test1/huawei-pc-manager-bootstrap-pcmanager14-test1-source.zip)。
- [SHA256SUMS.txt](https://github.com/mihahu/huawei-pc-manager-bootstrap/releases/download/pcmanager14-test1/SHA256SUMS.txt)：校验发布附件。

完整解压工具包，保持 EXE 和 DLL 在一起，以管理员身份运行启动器，选择自行从华为官网下载的电脑管家安装包。原版安装包和 HMS Core 不包含在附件中。如果提示先卸载或重启，完成后再安装；HMS Core 缺失时按电脑管家提示补齐。

## 改动与验证

- 保留上游 DLL 固定修复，补充初始化失败处理和诊断。
- 修复固件查询缓冲区边界、SMBIOS 解析和 SKU 字符串引用问题，保留 UUID 与其他记录。
- 55 次自动测试执行全部通过，涵盖 45 项不同测试；真实 DLL 转发、固件查询、子进程注入和模块生命周期探针通过。

本包保留用户已测试的 `0.1.9-883d64a` 二进制；其代码基线为 `883d64aa1fd02c229521abc254bfcf3a506077ee`。源码发布标签包含后续说明与 CI 配置，未更改运行代码。构建、架构和 SHA-256 详情见附件 `BUILD-INFO.json`。

已知限制：x86 核心无法直接注入部分 x64 辅助进程，可能记录 `ArchMismatch`；HMS Core 安装与账号服务也取决于实际环境。本项目不是华为官方工具。

## 作者来源与感谢

感谢 [hamflx/huawei-pc-manager-bootstrap](https://github.com/hamflx/huawei-pc-manager-bootstrap) 提供原始项目；感谢 [wornik](https://github.com/wornik) 在 [PR #54](https://github.com/hamflx/huawei-pc-manager-bootstrap/pull/54) 和 [v0.1.13-fix](https://github.com/wornik/huawei-pc-manager-bootstrap/releases/tag/v0.1.13-fix) 提供 DLL 生命周期修复。本版在这些成果上完成适配与验证，完整归属见仓库 `CREDITS.md`。
