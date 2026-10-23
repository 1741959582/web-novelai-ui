# 更新记录

从新到旧。安装包在 [最新发行版](https://github.com/caiweida/web-novelai-ui/releases/latest)。已安装的程序启动后会读到新版本，顶栏可以点「立即更新」。

## [1.0.10](https://github.com/caiweida/web-novelai-ui/releases/tag/v1.0.10)

- 历史里的图可以拖动换顺序。勾选多张后一起拖，滚轮也能滚动列表。
- DeepSeek 可以粘贴、拖入或选择图片，让模型看图回答。
- Pixiv 每天的浏览、点赞、评论、收藏和涨粉会记在本机，第二天不会把前几天清掉。
- 视频保存到本机时，代理握手失败会改走直连。生成记录留在播放器下面，点一条可以再看、再保存。
- 关掉「内容过滤」只是走 reAPI 的宽松通道，Seedance 自己的审核仍可能拒绝提示词或参考图。
- 脚本包更新到 0.1.44，里面带上 Pixiv 每日数据落盘。设置里检查脚本包即可热更新。

## [1.0.9](https://github.com/caiweida/web-novelai-ui/releases/tag/v1.0.9)

- 养号会真正启动操作脚本。广场打开首页「为你推荐」后开始滚动，并随机点赞、转发和关注。
- 勾选无头浏览器时，先关掉这个账号已经弹出的窗口，再按无头方式重新打开。
- 脚本包是 0.1.41，里面同时有无头启动和 Pixiv 通知修复。设置里检查脚本包即可热更新。

## [1.0.8](https://github.com/caiweida/web-novelai-ui/releases/tag/v1.0.8)

- 带下划线的标签会拆开再翻译，一次点很多「译」也不会挤在一起失败。
- 无头养号的实时预览能出画面。从别的页面切回来，正在养的号会重新连上 LIVE。
- 四个号正在养时，仍可以打开没在养的推特或 Pixiv。
- 脚本包更新到 0.1.43。设置里检查脚本包即可热更新。

## [1.0.7](https://github.com/caiweida/web-novelai-ui/releases/tag/v1.0.7)

- 养号勾选无头浏览器后，指纹浏览器不再弹出窗口。
- 广场养号打开首页后马上开始滚动，不再停在加载页面。
- 脚本包更新到 0.1.42。设置里检查脚本包即可热更新。

## [1.0.6](https://github.com/caiweida/web-novelai-ui/releases/tag/v1.0.6)

- 修复读取 Pixiv 点赞、关注和每日数据时出现 `SyntaxError: return not in function`。
- Pixiv 有新通知时，「数据」按钮上会出现小红点。列表在顶部时向下滚动可以刷新。
- 养号可以同时跑多个账号，也可以勾选无头浏览器。点赞、转发和关注按设定的分钟随机进行。
- 脚本包更新到 0.1.41。设置里检查脚本包即可热更新。

## [1.0.5](https://github.com/caiweida/web-novelai-ui/releases/tag/v1.0.5)

- V5 可以加参考插条，拖动和缩放角色图，底色可选白或黑。
- Pixiv 账号可以看点赞记录、关注记录和每日数据，并在这个账号的指纹浏览器里打开帖子。
- 代理可以在自由猫和 Clash Verge 之间切换。养号可以改去广场。
- 脚本包更新到 0.1.39。设置里检查脚本包即可热更新。

## [1.0.4](https://github.com/caiweida/web-novelai-ui/releases/tag/v1.0.4)

- 指纹浏览器的代理节点可以改选自由猫里的固定节点。不选就走本机直连，选中后浏览器走自由猫的本地端口。
- 脚本包可以单独启动。这个版本的发行页同时提供安装包和脚本包，设置里可以热更新脚本。

## [1.0.3](https://github.com/caiweida/web-novelai-ui/releases/tag/v1.0.3)

- 养号改成任务队列。每个账号一条任务，气泡里能看到剩余时间，可以单独取消，也可以全部取消。排队或养号中的浏览器不能再打开，完成或取消后才能打开。
- 养号会滚动时间线、打开帖子并随机点赞转发，到设定的分钟数就停止。敏感内容会先点「显示」，浏览量和账号提示会自动关掉。
- 推特和 Pixiv 可以同时打开，不再挤在同一个窗口里。
- 脚本包更新到 0.1.38。设置里检查脚本包即可热更新。

## [1.0.2](https://github.com/caiweida/web-novelai-ui/releases/tag/v1.0.2)

- 指纹浏览器可以登录并保存推特和 Pixiv 账号。推特可以养号，Pixiv 目前只有登录和保存指纹。
- 设置里可以检查本机指纹浏览器。没有 `electron/firefox/firefox.exe` 时，从 151-5 发布页下载并解压。
- 脚本包更新到 0.1.34。同一备注名可以分别用在推特和 Pixiv，打开窗口时会带上选中的代理节点。

## [1.0.1](https://github.com/caiweida/web-novelai-ui/releases/tag/v1.0.1)

- APNG 伪装每次导出或复制都会在右下角印一个新的 000~999 序号，同一张图重复发不容易被吞。聊天里仍是封面，点开才是真图。
- 图片工具新增添加杂色，可以调强度、颗粒和是否彩色，加完后保存到指定文件夹。
- GIF 可以复制成文件，到聊天窗口粘贴。
- 批量任务写到本机文件里，关掉窗口后再打开还在。编辑提示词时能看见权重颜色。

## [1.0.0](https://github.com/caiweida/web-novelai-ui/releases/tag/v1.0.0)

- 积木里每个关键词可以停用。词还留在列表里，生成时不会使用，再点启用会连同权重一起恢复。
- 生成页的大图可以点叉隐藏，再用「显示图片」打开。新图出来时会自动显示。
- 图片反推队列改用小缩略图。几十张图时切换回生成页不再卡住。

## [0.1.19](https://github.com/caiweida/web-novelai-ui/releases/tag/v0.1.19)

- 菜单新增标签超市，在窗口里打开 Danbooru 标签分类、预设和模型索引。
- 标签超市按浅色显示，正文不再铺在白底上看不见。
- 图片反推可以用 Ctrl+V 粘贴截图或复制的图片，一次一张加入队列。正在输入文字时仍只粘贴文字。

## [0.1.18](https://github.com/caiweida/web-novelai-ui/releases/tag/v0.1.18)

- 提示词权重大于 1 显示橙红色，等于或小于 1 显示蓝色。积木列表用同样的颜色。
- 提示词旁可以打开积木和翻译。翻译会处理中文、日文和韩文，并保留权重写法。
- 积木里没有中文的词可以点「译」。译文记进本地词库，下次直接显示。
- 标签扩展可以搜索词库、查看关联标签和百科。窗口单独弹出，搜不到时会留在面板里说明。

## [0.1.17](https://github.com/caiweida/web-novelai-ui/releases/tag/v0.1.17)

- 菜单新增视频生成：文生视频、参考图、首尾帧，使用 reAPI 的 Seedance。内容过滤默认关闭。
- 本机参考图先上传到图床，再用返回的图片地址生成。设置里可以保存 reAPI Key。
- 打马赛克可以分别设置马赛克颗粒和模糊程度，并一键恢复默认。

## [0.1.16](https://github.com/caiweida/web-novelai-ui/releases/tag/v0.1.16)

- 清除元数据和打马赛克可以倒序排列队列，也可以删除当前这张。

## [0.1.15](https://github.com/caiweida/web-novelai-ui/releases/tag/v0.1.15)

- V4.5 画风参考和角色参考按官网格式提交，不再出现 director references 的 400。
- 角色参考会缩放到官网的三种画布并补黑边；画风参考先编码再生成。

## [0.1.14](https://github.com/caiweida/web-novelai-ui/releases/tag/v0.1.14)

- 预览图上方可以做变体、超分和增强，费用按所选参数显示。
- Opus 在 1024×1024 以内、不超过 28 步时，重绘和普通生成一样不扣积分。
- 检查更新如果被 GitHub 接口限流，会改读发行页面，仍然能找到安装包。

## [0.1.13](https://github.com/caiweida/web-novelai-ui/releases/tag/v0.1.13)

- 打码模型批量下载时，某一个失败会自动继续下一个。
- 下载过程显示进度条和已下载大小。

## [0.1.12](https://github.com/caiweida/web-novelai-ui/releases/tag/v0.1.12)

- 检查更新会显示「检查中」，失败时写出原因，不再点了没反应。
- 当前版本不用等 GitHub 返回就能看到。连官方失败时会再走本机代理试一次。

## [0.1.11](https://github.com/caiweida/web-novelai-ui/releases/tag/v0.1.11)

- 主关键词和每个角色关键词旁可以打开 Danbooru 标签模糊搜索，把选中的标签写进对应的框。
- 生成页和批量页都可用，搜索按钮和标签按钮在同一行。

## [0.1.10](https://github.com/caiweida/web-novelai-ui/releases/tag/v0.1.10)

- 打码可以一次放入多张，自动检测后先放进队列，再用人工审核补漏或擦除。方式选模糊时，补笔也是模糊。
- 没有需要打码的图片也会按顺序保存，并清除元数据。
- 打码队列在切换页面或刷新后还在，点清空才会丢掉。
- 批量任务可以命名、保存，并切换回之前的任务。
- 加入图片时按文件名的数字顺序排列。

## [0.1.9](https://github.com/caiweida/web-novelai-ui/releases/tag/v0.1.9)

- 历史可以导入所选文件夹里所有带元数据的图片。
- 导入批量时会带上图片里的角色提示词。
- 批量队列可以全选，把某个提示词统一替换，完成后也能重新生成。
- 清除元数据可以拖动排序，保存的文件按顺序命名为 1.png、2.png。

## [0.1.8](https://github.com/caiweida/web-novelai-ui/releases/tag/v0.1.8)

- 清除元数据页可以自己选保存位置，清完的图片全部保存到这个文件夹。

## [0.1.7](https://github.com/caiweida/web-novelai-ui/releases/tag/v0.1.7)

- 清除元数据之后，可以点「保存到对应文件夹」，把这些图放进各自原来的分组文件夹。

## [0.1.6](https://github.com/caiweida/web-novelai-ui/releases/tag/v0.1.6)

- 选中分组后生成的新图会放进分组文件夹，同时仍显示在预览和该分组的历史里。

## [0.1.5](https://github.com/caiweida/web-novelai-ui/releases/tag/v0.1.5)

- 已分组的历史图片可以放进以分组名命名的文件夹。
- 选中的历史图可以导入批量任务，关键词之后可以自己改。

## [0.1.4](https://github.com/caiweida/web-novelai-ui/releases/tag/v0.1.4)

- 历史多选后，方框会打上勾，整行会有亮边，选中状态一眼能看出来。

## [0.1.3](https://github.com/caiweida/web-novelai-ui/releases/tag/v0.1.3)

- 历史记录可以多选，再批量加入 APNG 伪装、合成 GIF、还原真图、清除元数据、打马赛克。

## [0.1.2](https://github.com/caiweida/web-novelai-ui/releases/tag/v0.1.2)

- 复制伪装图时改为复制 APNG 文件本身，右键和「复制图片」不再贴出封面位图。
- 启动时读取 GitHub Release，有新版本可在顶栏或「设置 → 关于与更新」下载安装包。

## [0.1.1](https://github.com/caiweida/web-novelai-ui/releases/tag/v0.1.1)

- 修复安装版下载本地 CL Tagger 模型时连不上 huggingface.co、状态一直显示缺文件的问题。
- 连不上 Hugging Face 官网时会自动改试 hf-mirror.com；仍失败可在本地打标页填写代理。

## [0.1.0](https://github.com/caiweida/web-novelai-ui/releases/tag/v0.1.0)

- 第一个 Windows 安装包：安装版带安装向导和开始菜单快捷方式，便携版解压后运行 `web-novelai-ui.exe`。
- 第一次打开按向导填写 NovelAI Token。
