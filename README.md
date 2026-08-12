<p align="center">
  <a href="pjsekai-soundgen-rust"><strong>English</strong></a> | <a href="#rust%E7%89%88%E3%83%97%E3%83%AD%E3%82%BB%E3%82%AB%E9%A2%A8%E8%AD%9C%E9%9D%A2%E9%9F%B3%E5%A3%B0%E7%94%9F%E6%88%90%E3%83%84%E3%83%BC%E3%83%AB"><strong>日本語</strong></a>
</p>

<p align="center">
  <a href="https://github.com/kawawa-09/pjsekai-soundgen-rust/blob/main/LICENSE"><img src="https://img.shields.io/github/license/kawawa-09/pjsekai-soundgen-rust" alt="License"></a> 
  <a href="https://github.com/kawawa-09/pjsekai-soundgen-rust/releases/"><img src="https://img.shields.io/github/downloads/kawawa-09/pjsekai-soundgen-rust/total" alt="Releases"></a> 
  <a href="https://github.com/kawawa-09/pjsekai-soundgen-rust/stargazers"><img src="https://img.shields.io/github/stars/kawawa-09/pjsekai-soundgen-rust?style=flat&amp;color=yellow" alt="Stargazers"></a>
</p>

## pjsekai-soundgen-rust

[Latest Release](https://github.com/kawawa-09/pjsekai-soundgen-rust/releases/latest)

pjsekai-soundgen-rust is a tool that generates audio from various PJSK servers.


###  List of Servers
>
 > - `frpt-`: [Potato Leaves Archive](https://ptlv.milkbun.org/)
 > - `chcy-`: [Chart Cyanvas Archive](https://cc.milkbun.org/)
 > - `local-`: ScoreSync

> [!WARNING]
> The following servers are not currently supported
>
>  > [Chart Cyanvas Fork Server](https://chart-cyanvas.com/)
>
> >  ScoreSync Modern

> [!CAUTION]
> ## Prerequisites
> - [ffmpeg in your PATH](https://ffmpeg.org/)


### How to Use

0. Install ffmpeg.
1. Download pjsekai-soundgen-rust.zip from [Releases](https://github.com/sevenc-nanashi/pjsekai-soundgen-rust/releases).
3. Enter the prefix-{chart ID}.

> [!NOTE]
> For most servers, enter `prefix-{chart ID}`
 > - [Potato Leaves Archive `frpt-` ](https://ptlv.milkbun.org/)
 > - [Chart Cyanvas Archive `chcy-` ](https://cc.milkbun.org/)
 > - ScoreSync `local-`

> [!TIP]
> For ScoreSync, enter `local-{chart filename}`

4. The results will be generated in the `dist` directory

Please check the [wiki](https://github.com/kawawa-09/pjsekai-soundgen-rust/wiki#english) for more details.

### Terms of Use

Please include the following information about me (=Anonymous.) in the video description or similar section:
```
- Name (Anonymous.)
- A link to this repository
- A link to https://sevenc7c.com
```

### Example

```
Proseca-style chart audio generation tool:
  https://github.com/sevenc-nanashi/pjsekai-soundgen-rust
  Created by: Anonymous. ( https://sevenc7c.com )
  https://github.com/Piliman22/pjsekai-soundgen-rust
  Forked by: Piman ( https://pim4n-net.com/ )
  https://github.com/kawawa-09/pjsekai-soundgen-rust
  Forked by: Kawarisu ( https://www.youtube.com/@kawa-risu )
```
### TODO
 - Next Sekai Engine
 - Implement Next Rush and + Engine
> - Servers planned for support
>  - `UnCh-`: [UntitledCharts](https://untitledcharts.com/)
>  - `sss-`: [Sbuga’s Sonolus Server](https://sonolus.sbuga.com/)
>  - `sekai-best-`: [Sekai Viewer](https://sonolus.sekai.best/)

### License

The source code is released under the GPLv3.

Translated with DeepL.com (free version)

## Rust版プロセカ風譜面音声生成ツール

pjsekai-soundgen-rust は、様々なPJSKサーバーから音声を生成するツールです。

[最新リリース](https://github.com/kawawa-09/pjsekai-soundgen-rust/releases/latest)

###  サーバー一覧
> 
 > - `frpt-`：[Potato Leaves Archive](https://ptlv.milkbun.org/)
 > - `chcy-`：[Chart Cyanvas Archive](https://cc.milkbun.org/)
 > - `local-` ：ScoreSync

> [!WARNING]
> 以下のサーバーは現在は対応していません
> 
>  > [Chart Cyanvas  分岐サーバー](https://chart-cyanvas.com/)
> 
> >  ScoreSync Modern

> [!CAUTION]
> ## 必須事項
> - [PATH 上の ffmpeg](https://ffmpeg.org/)


### 利用方法

0. ffmpeg をインストールする。
1. [Releases](https://github.com/sevenc-nanashi/pjsekai-soundgen-rust/releases)からpjsekai-soundgen-rust.zipをダウンロードする。
3.接頭辞-{譜面ID}と入力する。

> [!NOTE]  
> ほとんどのサーバーの場合`接頭辞-{譜面ID}`と入力
 > - [Potato Leaves Archive `frpt-` ](https://ptlv.milkbun.org/)
 > - [Chart Cyanvas Archive `chcy-` ](https://cc.milkbun.org/)
 > - ScoreSync `local-` 

> [!TIP]
> ScoreSyncの場合`local-{譜面ファイル名}`と入力

4. dist内に結果が生成される

詳しくは[wiki](https://github.com/kawawa-09/pjsekai-soundgen-rust/wiki#%E6%97%A5%E6%9C%AC%E8%AA%9E)を確認してください

### 利用規約

動画の概要欄などに、自分（=名無し｡）の
```
- 名前（名無し｡）
- このリポジトリへのリンク
- https://sevenc7c.com へのリンク
```
が含まれている文章を載せて下さい。

### 例

```
プロセカ風譜面音声生成ツール：
  https://github.com/sevenc-nanashi/pjsekai-soundgen-rust
  作成：名無し｡ （ https://sevenc7c.com ）
  https://github.com/Piliman22/pjsekai-soundgen-rust
  フォーク：ぴぃまん　( https://pim4n-net.com/ )
　https://github.com/kawawa-09/pjsekai-soundgen-rust
  フォーク：Kawarisu　（　https://www.youtube.com/@kawa-risu　）
```
### TODO
 - Next Sekaiエンジン
 - Next Rush , + エンジンを実装する
> - 対応予定のあるサーバー
>  - `UnCh-`：[UntitledCharts](https://untitledcharts.com/)
>  - `sss-`：[SbugaのSonolusサーバー](https://sonolus.sbuga.com/)
>  - `sekai-best-`：[Sekai Viewer](https://sonolus.sekai.best/)

### ライセンス

ソースコードはGPLv3で公開されています。
