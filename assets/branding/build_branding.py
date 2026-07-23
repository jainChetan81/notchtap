# notchtap branding build script
# trims the winning concept, exports icon sizes, og-image, and lockups.
from PIL import Image, ImageDraw, ImageFont
import numpy as np
import os

ROOT = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.join(ROOT, "concept-a-notch-card.png")

BG = (5, 6, 7, 255)          # #050607
TEXT = (241, 242, 244, 255)  # #f1f2f4
FONT = "/System/Library/Fonts/HelveticaNeue.ttc"
FONT_IDX = 1                 # helvetica neue bold

im = Image.open(SRC).convert("RGBA")
a = np.array(im.getchannel("A"), dtype=np.uint8)
h, w = a.shape

# erase the generator watermark (bottom-left corner)
a[int(h * 0.88):, : int(w * 0.3)] = 0
im.putalpha(Image.fromarray(a))

# tight content bbox with alpha threshold to ignore faint halo
mask = a > 40
ys, xs = np.where(mask)
x0, y0, x1, y1 = xs.min(), ys.min(), xs.max() + 1, ys.max() + 1
print("content bbox:", x0, y0, x1, y1)
content = im.crop((x0, y0, x1, y1))
cw, ch = content.size

# square canvas, content fills ~88% of the side
side = int(max(cw, ch) / 0.88)
sq = Image.new("RGBA", (side, side), (0, 0, 0, 0))
sq.paste(content, ((side - cw) // 2, (side - ch) // 2), content)

master = sq.resize((1024, 1024), Image.LANCZOS)
master.save(os.path.join(ROOT, "notchtap-mark-1024.png"))
for s in (32, 64, 128, 256, 512):
    sq.resize((s, s), Image.LANCZOS).save(
        os.path.join(ROOT, f"notchtap-mark-{s}.png"))
print("icon sizes written")


def make_lockup(height, bg=None, path=None):
    """premier-league-style horizontal lockup: mark left, wordmark right."""
    pad = int(height * 0.14)          # vertical padding
    mark_h = height - 2 * pad
    mark = sq.resize((mark_h, mark_h), Image.LANCZOS)

    font_size = int(height * 0.52)
    font = ImageFont.truetype(FONT, font_size, index=FONT_IDX)
    word = "notchtap"
    # measure text
    tmp = Image.new("RGBA", (10, 10))
    d = ImageDraw.Draw(tmp)
    tb = d.textbbox((0, 0), word, font=font)
    tw, th = tb[2] - tb[0], tb[3] - tb[1]

    gap = int(height * 0.22)
    width = pad + mark_h + gap + tw + pad
    canvas = Image.new("RGBA", (width, height), bg if bg else (0, 0, 0, 0))
    canvas.paste(mark, (pad, pad), mark)

    d = ImageDraw.Draw(canvas)
    # baseline-center the text against the mark's optical center
    tx = pad + mark_h + gap
    ty = (height - th) // 2 - tb[1]
    d.text((tx, ty), word, font=font, fill=TEXT)
    canvas.save(path)
    print("lockup:", os.path.basename(path), canvas.size)
    return canvas


for hh in (128, 256, 512):
    make_lockup(hh, path=os.path.join(ROOT, f"notchtap-lockup-{hh}.png"))
# dark-background variant at 512
make_lockup(512, bg=BG, path=os.path.join(ROOT, "notchtap-lockup-512-dark.png"))

# og-image 1200x630 on #050607: centered lockup group
og = Image.new("RGBA", (1200, 630), BG)
lk = make_lockup(280, path=os.path.join(ROOT, "_tmp-lockup-og.png"))
og.paste(lk, ((1200 - lk.width) // 2, (630 - lk.height) // 2), lk)
og.convert("RGB").save(os.path.join(ROOT, "notchtap-og-1200x630.png"))
os.remove(os.path.join(ROOT, "_tmp-lockup-og.png"))
print("og-image written")
