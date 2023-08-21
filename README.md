# tvnow
## Description
現在放送中のTV番組や1週間分の番組などを表示します
## Usage
```bash
tvnow
```
デフォルトの視聴エリアは`tokyo`  
BS放送は`bs`  
環境変数`TV_AREA`でデフォルトを変更できます
```bash
$ export TV_AREA=osaka
```

```
tvnow 0.1.0
tv program display

USAGE:
    tvnow [FLAGS] [AREA]...

FLAGS:
    -a, --area       Prints area list
    -h, --help       Prints help information
    -t, --today      Prints today's program
    -V, --version    Prints version information
    -w, --week       Prints a week program

ARGS:
    <AREA>...
```
## Example

```bash
tvnow --today sapporo
```
```bash
tvnow --week bs | less
```
```bash
tvnow -w | grep 🈙
```