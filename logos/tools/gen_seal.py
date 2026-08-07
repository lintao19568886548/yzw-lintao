#!/usr/bin/env python3
"""生成「云园慧控」印面 logo：小篆「云」+ 四角边角云纹。

云纹不是画的，是把用户提供的参考图（传统边角云纹）用 potrace 矢量化后
烘进坐标得到的，形态忠于原图。

流程：JPEG --magick--> 位图 --potrace--> SVG 路径 --本脚本--> 归一化 + 摆四角

用法：python3 gen_seal.py [参考图路径]
输出：../concepts/concept-25.svg（镜像四角）
      ../concepts/concept-26.svg（旋转四角）
"""
import os
import re
import subprocess
import sys
import tempfile

INK = "#f2ece0"    # 宣白
PLATE = "#9c3a2b"  # 朱砂

REF_DEFAULT = ("/Users/ruster/.claude/uploads/"
               "fa816fd1-6e61-4caf-b6a7-b4225b74d8c7/"
               "e46281f5-d145217af6622bb7b1fd1df22ca9bfa192751173.jpeg")

# —— 印面参数 ——
PLATE_XY, PLATE_WH, PLATE_R = 48, 416, 10
CLOUD_SIZE = 132          # 云纹方框边长（印面 416 的约 32%）
GLYPH_SCALE, GLYPH_STROKE = 0.84, 24

# 仿小篆「云」：二横在上，下作云气回转
GLYPH = [
    ("heng-1", "M148 140H364"),
    ("heng-2", "M176 200H336"),
    ("huizhuan", "M196 258c-18 106 44 176 120 138 46-23 44-82 2-100"
                 "-30-13-58 6-54 36 3 21 24 29 38 16"),
]

NUM = re.compile(r"[-+]?[0-9]*\.?[0-9]+(?:[eE][-+]?[0-9]+)?")
CMD = re.compile(r"([MmLlHhVvCcSsQqTtAaZz])")


def trace(ref):
    """参考图 → potrace SVG（路径 d + potrace 的 translate/scale）。"""
    tmp = tempfile.mkdtemp()
    pbm, out = os.path.join(tmp, "r.pbm"), os.path.join(tmp, "r.svg")
    subprocess.run(["magick", ref, "-colorspace", "Gray", "-resize", "1200x",
                    "-threshold", "55%", "-morphology", "Open", "Disk:1.5",
                    "-bordercolor", "white", "-border", "12", pbm], check=True)
    subprocess.run(["potrace", "-s", "--alphamax", "1.0",
                    "--opttolerance", "0.2", "-o", out, pbm], check=True)
    svg = open(out).read()
    d = re.search(r'<path d="(.*?)"', svg, re.S).group(1)
    tx, ty = map(float, re.search(r"translate\(([-\d.]+),([-\d.]+)\)", svg).groups())
    sx, sy = map(float, re.search(r"scale\(([-\d.]+),([-\d.]+)\)", svg).groups())
    return d, (tx, ty, sx, sy)


def parse(d):
    """路径 d → 子路径列表，每条为绝对坐标的 [(cmd, pts...)]。"""
    toks = [t for t in CMD.split(d) if t.strip()]
    subs, cur = [], []
    x = y = sx = sy = 0.0
    i = 0
    while i < len(toks):
        c = toks[i]
        args = [float(v) for v in NUM.findall(toks[i + 1])] if i + 1 < len(toks) \
            and not CMD.fullmatch(toks[i + 1]) else []
        i += 2 if args else 1
        rel = c.islower()
        u = c.upper()
        if u == "M":
            for j in range(0, len(args), 2):
                px, py = args[j], args[j + 1]
                if rel:
                    px, py = x + px, y + py
                if j == 0:
                    if cur:
                        subs.append(cur)
                    cur = [("M", (px, py))]
                    sx, sy = px, py
                else:
                    cur.append(("L", (px, py)))
                x, y = px, py
        elif u == "L":
            for j in range(0, len(args), 2):
                px, py = args[j], args[j + 1]
                if rel:
                    px, py = x + px, y + py
                cur.append(("L", (px, py)))
                x, y = px, py
        elif u == "H":
            for v in args:
                px = x + v if rel else v
                cur.append(("L", (px, y)))
                x = px
        elif u == "V":
            for v in args:
                py = y + v if rel else v
                cur.append(("L", (x, py)))
                y = py
        elif u == "C":
            for j in range(0, len(args), 6):
                p = [(args[j + k], args[j + k + 1]) for k in (0, 2, 4)]
                if rel:
                    p = [(x + a, y + b) for a, b in p]
                cur.append(("C", *p))
                x, y = p[2]
        elif u == "Z":
            cur.append(("Z",))
            x, y = sx, sy
    if cur:
        subs.append(cur)
    return subs


def bbox(subs):
    xs = [p[0] for s in subs for seg in s for p in seg[1:]]
    ys = [p[1] for s in subs for seg in s for p in seg[1:]]
    return min(xs), min(ys), max(xs), max(ys)


def emit(subs, fn):
    out = []
    for s in subs:
        for seg in s:
            if seg[0] == "Z":
                out.append("Z")
            else:
                pts = " ".join("%.2f %.2f" % fn(*p) for p in seg[1:])
                out.append(seg[0] + pts)
    return "".join(out)


def normalize(d, pot, size):
    """烘进 potrace 变换、归一化到 size 见方、转成左上角朝向的角饰。"""
    tx, ty, sx, sy = pot
    subs = parse(d)
    subs = [[(seg[0],) + tuple((tx + sx * p[0], ty + sy * p[1]) for p in seg[1:])
             for seg in s] for s in subs]
    x0, y0, x1, y1 = bbox(subs)
    k = size / max(x1 - x0, y1 - y0)
    # 原图实心角在左下；旋转 90° 使实心角落到原点，两臂沿 +x／+y 展开
    return emit(subs, lambda px, py: (size - (py - y0) * k, (px - x0) * k))


def build(cloud_d, mode):
    x0 = y0 = PLATE_XY
    x1 = y1 = PLATE_XY + PLATE_WH
    if mode == "mirror":
        placements = [f"translate({x0} {y0})", f"translate({x1} {y0}) scale(-1 1)",
                      f"translate({x1} {y1}) scale(-1 -1)",
                      f"translate({x0} {y1}) scale(1 -1)"]
    else:
        placements = [f"translate({x0} {y0})", f"translate({x1} {y0}) rotate(90)",
                      f"translate({x1} {y1}) rotate(180)",
                      f"translate({x0} {y1}) rotate(270)"]
    uses = "\n".join('    <use href="#corner-cloud" transform="%s"/>' % p
                     for p in placements)
    glyph = "\n".join('      <path id="%s" d="%s"/>' % g for g in GLYPH)
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512">
  <!-- 云园慧控 · 小篆「云」朱砂方印 + 四角边角云纹（{mode} 布局） -->
  <!-- 云纹取自参考图矢量化结果；由 logos/tools/gen_seal.py 生成，勿手改 -->
  <!-- 传统色：朱砂 {PLATE} / 宣白 {INK} -->
  <defs>
    <clipPath id="plateClip">
      <rect x="{PLATE_XY}" y="{PLATE_XY}" width="{PLATE_WH}" height="{PLATE_WH}" rx="{PLATE_R}"/>
    </clipPath>
    <path id="corner-cloud" fill="{INK}" d="{cloud_d}"/>
  </defs>

  <g id="icon" clip-path="url(#plateClip)">
    <rect id="plate" x="{PLATE_XY}" y="{PLATE_XY}" width="{PLATE_WH}" height="{PLATE_WH}" rx="{PLATE_R}" fill="{PLATE}"/>
{uses}
    <g id="glyph-yun" transform="translate(256 252) scale({GLYPH_SCALE}) translate(-256 -270)"
       fill="none" stroke="{INK}" stroke-width="{GLYPH_STROKE}"
       stroke-linecap="round" stroke-linejoin="round">
{glyph}
    </g>
  </g>
</svg>
'''


if __name__ == "__main__":
    ref = sys.argv[1] if len(sys.argv) > 1 else REF_DEFAULT
    d, pot = trace(ref)
    cloud = normalize(d, pot, CLOUD_SIZE)
    here = os.path.dirname(os.path.abspath(__file__))
    for name, mode in (("concept-25", "mirror"), ("concept-26", "rotate")):
        p = os.path.normpath(os.path.join(here, "..", "concepts", name + ".svg"))
        open(p, "w").write(build(cloud, mode))
        print("wrote", p)
