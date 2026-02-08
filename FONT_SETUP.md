# 字体设置说明 / Font Setup Instructions

## 问题 / Issue

当前 `assets/NotoSansSC-Regular.otf` 字体文件已损坏（实际上是一个 HTML 文件而不是字体文件）。为了防止应用启动时崩溃，自定义字体加载功能已被临时禁用。

The current `assets/NotoSansSC-Regular.otf` font file is corrupted (it's actually an HTML file instead of a font file). To prevent the application from crashing on startup, the custom font loading feature has been temporarily disabled.

## 当前状态 / Current Status

应用将使用系统默认字体运行。在 Windows 11 上，系统默认字体通常能够正确显示中文字符。

The application will run using system default fonts. On Windows 11, the system default fonts can usually display Chinese characters correctly.

## 如何启用自定义字体 / How to Enable Custom Fonts

如果您希望使用自定义中文字体以获得更好的显示效果，请按以下步骤操作：

If you want to use a custom Chinese font for better display, follow these steps:

### 步骤 / Steps

1. **下载有效的 Noto Sans SC 字体文件**
   
   Download a valid Noto Sans SC font file:
   - 访问 Google Fonts: https://fonts.google.com/noto/specimen/Noto+Sans+SC
   - 点击 "Download family" 下载字体包
   - 解压下载的 ZIP 文件
   - 找到 `NotoSansSC-Regular.otf` 或 `NotoSansSC-Regular.ttf` 文件

2. **替换损坏的字体文件**
   
   Replace the corrupted font file:
   ```bash
   # 删除损坏的字体文件
   # Delete the corrupted font file
   rm assets/NotoSansSC-Regular.otf
   
   # 复制下载的有效字体文件到 assets 目录
   # Copy the downloaded valid font file to the assets directory
   cp /path/to/downloaded/NotoSansSC-Regular.otf assets/
   ```

3. **启用字体加载代码**
   
   Enable the font loading code:
   - 打开 `src/main.rs` 文件
   - 找到 `setup_custom_fonts` 函数
   - 取消注释字体加载代码块（删除 `/*` 和 `*/` 标记）
   - 保存文件

4. **重新编译并运行**
   
   Rebuild and run:
   ```bash
   cargo build --release
   cargo run --release
   ```

## 验证字体 / Verify Font

在替换字体文件后，可以使用 `file` 命令验证文件类型：

After replacing the font file, you can verify the file type using the `file` command:

```bash
file assets/NotoSansSC-Regular.otf
```

应该输出类似：
Should output something like:
```
assets/NotoSansSC-Regular.otf: OpenType font data
```

如果输出包含 "HTML" 或其他非字体类型，说明文件仍然不正确。

If the output contains "HTML" or other non-font types, the file is still incorrect.

## 故障排除 / Troubleshooting

### 应用仍然崩溃 / Application Still Crashes

如果替换字体后应用仍然崩溃，请：

If the application still crashes after replacing the font:

1. 检查字体文件是否真的是有效的字体文件（使用 `file` 命令）
   Check if the font file is really a valid font file (use `file` command)
2. 尝试使用不同的字体文件
   Try using a different font file
3. 保持自定义字体代码为注释状态，使用系统默认字体
   Keep the custom font code commented out and use system default fonts

### 中文显示为方框 / Chinese Characters Display as Boxes

如果使用系统默认字体时中文显示为方框：

If Chinese characters display as boxes when using system default fonts:

1. 确保您的操作系统已安装中文字体
   Ensure your operating system has Chinese fonts installed
2. 尝试按照上述步骤启用自定义字体
   Try enabling custom fonts following the steps above
3. 在 Windows 上，确保已安装中文语言包
   On Windows, make sure Chinese language pack is installed
