# ZipX Windows Installer 构建指南

## 📋 前置要求

### 必需软件
1. **Node.js** (v16或更高)
   - 下载地址: https://nodejs.org/
   - 安装后重启命令行

2. **Rust** (最新稳定版)
   - 下载地址: https://www.rust-lang.org/
   - 安装后重启命令行

3. **Git** (可选，用于克隆代码)
   - 下载地址: https://git-scm.com/

## 🔨 构建步骤

### 方法一：自动构建脚本（推荐）

1. **打开命令提示符（CMD）或PowerShell**

2. **进入项目根目录**
   ```cmd
   cd E:\ZipX
   ```

3. **运行构建脚本**
   ```cmd
   build-installer.bat
   ```

4. **等待构建完成**
   - 首次构建需要5-10分钟
   - 构建成功后会显示安装包位置

### 方法二：手动构建

#### 步骤1：安装Node依赖
```cmd
cd ui
npm install
```

#### 步骤2：构建Tauri应用
```cmd
npm run tauri build
```

#### 步骤3：查找构建产物
构建完成后，应用程序和安装包位于：
- **可执行文件**: `ui\src-tauri\target\release\zipx-ui.exe`
- **安装包**: `ui\src-tauri\target\release\bundle\`

## 📦 构建产物

### 主应用程序
- **位置**: `ui/src-tauri/target/release/zipx-ui.exe`
- **大小**: 约5-10 MB
- **类型**: 独立Windows可执行文件

### 安装包格式

#### NSIS安装程序（推荐）
- **位置**: `ui/src-tauri/target/release/bundle/nsis/`
- **文件**: `ZipX_0.1.0_x64-setup.exe`
- **说明**: 双击安装，包含卸载程序

#### MSI安装包
- **位置**: `ui/src-targi/target/release/bundle/msi/`
- **文件**: `ZipX_0.1.0_x64_en-US.msi`
- **说明**: Windows标准安装包

## 🎨 iOS风格UI特性

### 已实现的iOS风格设计
- ✨ **毛玻璃效果** - 半透明背景，模糊效果
- 🎯 **大圆角** - 16-24px圆角设计
- 🌈 **优雅渐变** - 蓝色/紫色渐变按钮
- 💫 **流畅动画** - 淡入、滑动、缩放效果
- 🎨 **SF Pro字体** - Apple系统字体风格
- 📱 **分段控制器** - iOS风格模式切换
- 📂 **文件选择器** - 图标+文字组合
- ✅ **成功状态** - 绿色勾选图标显示
- ⚠️ **错误提示** - 红色错误消息

### 交互改进
- 🖱️ **原生文件选择器** - Tauri dialog API
- 📁 **文件夹选择** - 原生目录选择
- 🔄 **自动格式检测** - 实时显示检测到的格式
- 📊 **结果卡片** - 毛玻璃效果的结果展示
- 🎭 **状态动画** - 加载旋转、成功勾选
- ⌨️ **键盘友好** - 支持Tab导航

## 🐛 故障排除

### 常见问题

#### 1. npm命令未找到
**错误**: 'npm' is not recognized
**解决**:
- 安装Node.js: https://nodejs.org/
- 重启命令提示符
- 验证安装: `npm --version`

#### 2. Rust编译失败
**错误**: error: linker `link.exe` not found
**解决**:
- 安装Microsoft C++ Build Tools
- 运行: `rustup update`
- 或安装Visual Studio Community (包含C++工具)

#### 3. Tauri构建失败
**错误**: "package does not contain this feature: custom-protocol"
**解决**:
- 删除 `ui/src-tauri/Cargo.lock`
- 删除 `ui/node_modules` 文件夹
- 重新运行: `npm install`
- 重新运行: `npm run tauri build`

#### 4. Vite构建警告
**警告**: A11y: visible, non-interactive elements with on:click
**说明**: 这是可访问性警告，不影响功能
**可选修复**: 添加role="button"和tabindex="0"属性

#### 5. 构建缓慢
**说明**: 首次构建需要下载Rust依赖（~500MB）
**建议**:
- 确保网络连接稳定
- 耐心等待，后续构建会快很多
- 可以使用缓存加速: `cargo build --release` (核心库)

## 🚀 快速启动开发模式

### 开发模式（热重载）
```cmd
cd ui
npm install
npm run tauri dev
```

这会启动：
- Vite开发服务器（热重载）
- Tauri原生窗口
- Rust后端（自动重新编译）

### 构建生产版本
```cmd
cd ui
npm run tauri build
```

## 📦 分发准备

### 创建便携版
直接分发 `zipx-ui.exe` 可执行文件，用户无需安装。

### 创建安装包
使用构建生成的NSIS或MSI安装包：
- 包含应用程序
- 自动创建桌面快捷方式
- 添加到开始菜单
- 支持卸载

## 🎯 自定义配置

### 修改应用图标
1. 准备 `.ico` 文件 (256x256像素)
2. 放置在 `ui/src-tauri/icons/`
3. 更新 `tauri.conf.json` 中的图标路径

### 修改应用信息
编辑 `ui/src-tauri/tauri.conf.json`:
```json
{
  "package": {
    "productName": "ZipX",
    "version": "0.1.0"
  },
  "tauri": {
    "windows": [{
      "title": "ZipX",
      "width": 1200,
      "height": 800,
      "resizable": true,
      "fullscreen": false
    }]
  }
}
```

### 修改窗口大小和样式
编辑 `ui/src/App.svelte` 中的CSS:
- 调整 `max-width` 控制宽度
- 修改 `background` 改变背景色
- 更改 `border-radius` 调整圆角

## 📋 性能优化建议

### 减小安装包大小
- 使用 `cargo build --release` 优化Rust代码
- 启用LTO (Link Time Optimization)
- 压缩资源文件

### 提升启动速度
- 延迟加载非关键模块
- 优化数据库查询
- 缓存常用操作

## 🔐 代码签名（可选）

### Windows代码签名
1. 获取代码签名证书
2. 安装SignTool
3. 对可执行文件进行签名
4. 验证签名

## 📝 构建验证

### 检查清单
- [ ] 可执行文件可以正常运行
- [ ] 文件选择器功能正常
- [ ] 压缩和解压功能正常
- [ ] 自动格式检测工作
- [ ] UI显示正确，无错位
- [ ] 安装包可以在干净系统上安装
- [ ] 卸载功能正常

## 🆕 获取帮助

### 官方文档
- Tauri: https://tauri.app/
- Svelte: https://svelte.dev/
- Rust: https://www.rust-lang.org/

### 社区支持
- GitHub Issues: 在项目仓库提交问题
- Discord: 加入Tauri社区Discord

## 🎉 下一步

构建完成后：
1. 运行 `zipx-ui.exe` 测试基本功能
2. 创建安装包并测试安装流程
3. 准备分发材料（截图、介绍等）
4. 发布到GitHub Releases或其他平台

祝构建顺利！🚀
