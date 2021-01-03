# lucile

```
ffmpeg -ss 675.8 -t 10.3 -i ~/source.mkv -filter_complex "[0:v] fps=12,scale=w=480:h=-1, subtitles=sk.srt:force_style='Fontsize=28',split [a][b];[a] palettegen=stats_mode=single:reserve_transparent=false [p];[b][p] paletteuse=new=1" -y ~/out.gif
```

start time
duration
input video
fps
width
subs file (generated to match start time)
font size
output file
```
ffmpeg -ss ${START_TIME} -t ${DURATION} -i ${INPUT_VIDEO} -filter_complex "[0:v] fps=${FPS},scale=w=${WIDTH}:h=-1, subtitles=${SUBS_FILE}:force_style='Fontsize=${FONT_SIZE}',split [a][b];[a] palettegen=stats_mode=single:reserve_transparent=false [p];[b][p] paletteuse=new=1" -y ${OUTPUT_FILE}
```

https://aws.amazon.com/blogs/media/processing-user-generated-content-using-aws-lambda-and-ffmpeg/
https://intoli.com/blog/transcoding-on-aws-lambda/