# 更新记录

从新到旧。安装包在 [最新发行版](https://github.com/caiweida/web-novelai-ui/releases/latest)。已安装的程序启动后会读到新版本，顶栏可以点「立即更新」。

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
