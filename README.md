# web-novelai-ui

中文 NovelAI 图像创作工作台。第一次打开按下面做即可。

## 第一次打开

启动后会弹出三步向导。

| 步骤 | 做什么 | 能不能跳过 |
| --- | --- | --- |
| 1. NovelAI Token | 粘贴 Persistent API Token（`pst-` 开头），点「验证」 | 可以先下一步。不填不能生图，之后到「设置 → API 配置」再填 |
| 2. 本地打标 | 检测 NVIDIA 显卡。显存够才允许下载并启用本地 CL Tagger v2 | 可以下一步。不启用时，反推仍可用 WD Tagger 或 Hugging Face 上的 CL Tagger |
| 3. 进入应用 | 点「进入应用」 | 会记住已经看过向导，下次不再弹出 |

Token 只写在本机，不要贴到截图或聊天里。

## 设置里哪些要填

打开左侧「设置」。**只有 Token 必须填**，其余保持默认。

| 设置 | 要不要填 | 说明 |
| --- | --- | --- |
| API 配置 → Token | 要 | 生图唯一必填。填完点「验证 Token / 刷新积分」，能看到订阅档位和 Anlas |
| 存储与网络 → 输出目录 | 不用 | 留空就保存到「图片 / Langbai NovelAI」 |
| 存储与网络 → Image API | 不用 | 保持官方地址。只有自建转发时才改 |
| 存储与网络 → HTTP / SOCKS 代理 | 不用 | 留空即直连。官方接口打不开时再填，例如 `http://127.0.0.1:7890` |
| 允许非官方端点、401/403 回退 | 不用 | 用官方接口时保持关闭 |
| 提示词补全 → 下载标签库 | 不用 | 不下载就用内置精简词库 |
| 本地打标 → CL Tagger | 不用 | 需要本机 NVIDIA 显卡，权重另外下载。不启用不影响生图 |
| 性能 → 流式预览 | 不用改 | 默认开着。失败会自动改用整张结果 |

改完「存储与网络」要点「保存设置」。

参考桌面版第一次启动还会提到这些，本项目没有对应设置，不用去配：

| 参考项目里的项 | 本项目怎么处理 |
| --- | --- |
| 设置 → AI 反推（视觉模型地址、Key、模型名） | 反推页直接选 WD Tagger 或 CL Tagger v2 |
| 设置 → 转换 API | 提示词工具使用内置词库和翻译 |
| 百度翻译 APP ID / 密钥 | 翻译不需要单独申请密钥 |
| 自动跟随系统代理开关 | 代理框留空就是直连 |

## 生成第一张图

1. 确认「设置 → API 配置」里 Token 已验证。
2. 打开「生成」，模型选账号能用的版本。角色提示词只在 V4 及以上可用。
3. 提示词可以先用：

```text
1girl, solo, blue hair, blue eyes, white dress, garden, sunlight, smile
```

4. 张数设为 1，看一眼尺寸和预计消耗，再点生成。
5. 图片出现在中间预览，并写入输出目录和右侧历史。

## 反推（可选）

1. 打开「反推」，加入图片，选 WD Tagger 或 CL Tagger v2。
2. WD 走公开空间，一般不用额外填写。
3. CL Tagger：设置里启用了本地模型就用本机显卡；没启用就走官方空间。匿名额度用完后，在「设置 → 本地打标」填 Hugging Face read token，再重试。

## 连不上时

| 现象 | 先看 |
| --- | --- |
| Token 验证失败或 401 | Token 是否完整、是不是 `pst-` 开头。不要粘贴浏览器 Cookie |
| 余额或模型不可用 | 账号权限、Anlas，以及当前尺寸和张数 |
| 超时、连接重置 | 先试直连；仍失败再填代理 |
| 反推没有标签 | 看是额度用完还是本地模型未下载 |

## 开发

与 [Tauri v2 Windows 前置要求](https://v2.tauri.app/zh-cn/start/prerequisites/) 一致：Visual Studio C++ 桌面开发、WebView2、`rustup default stable-msvc`、Node.js ≥ 18。

```bash
npm install
npm run dev
```

打包：`npm run build`（NSIS，见 `src-tauri/tauri.conf.json`）。
