import os
from PIL import Image

def generate_android_icons():
    # Base logo
    logo_path = "/Users/chinmayk/Downloads/ClipRelay-main/logo.png"
    base_img = Image.open(logo_path).convert("RGBA")
    
    # Adaptive Icon Foreground specs (Total size, Safe zone size)
    adaptive_specs = {
        "mipmap-mdpi": (108, 72),
        "mipmap-hdpi": (162, 108),
        "mipmap-xhdpi": (216, 144),
        "mipmap-xxhdpi": (324, 216),
        "mipmap-xxxhdpi": (432, 288),
    }
    
    # Legacy Icon specs (Total size)
    legacy_specs = {
        "mipmap-mdpi": 48,
        "mipmap-hdpi": 72,
        "mipmap-xhdpi": 96,
        "mipmap-xxhdpi": 144,
        "mipmap-xxxhdpi": 192,
    }
    
    res_dir = "/Users/chinmayk/Downloads/ClipRelay-main/platforms/android/app/src/main/res"
    
    for density, (total_size, safe_size) in adaptive_specs.items():
        out_dir = os.path.join(res_dir, density)
        os.makedirs(out_dir, exist_ok=True)
        
        # Create transparent canvas
        canvas = Image.new("RGBA", (total_size, total_size), (255, 255, 255, 0))
        
        # Resize logo to fit inside safe zone
        # We make it slightly smaller than safe zone to look good, e.g. 90% of safe zone
        draw_size = int(safe_size * 0.9)
        resized_logo = base_img.resize((draw_size, draw_size), Image.Resampling.LANCZOS)
        
        # Center it
        offset = (total_size - draw_size) // 2
        canvas.paste(resized_logo, (offset, offset), resized_logo)
        
        # Save foreground
        fg_path = os.path.join(out_dir, "ic_launcher_foreground.png")
        canvas.save(fg_path)
        print(f"Generated {fg_path}")
        
    for density, size in legacy_specs.items():
        out_dir = os.path.join(res_dir, density)
        os.makedirs(out_dir, exist_ok=True)
        
        # For legacy, just resize to exact size (with maybe a tiny bit of padding)
        draw_size = int(size * 0.9)
        canvas = Image.new("RGBA", (size, size), (255, 255, 255, 0))
        resized_logo = base_img.resize((draw_size, draw_size), Image.Resampling.LANCZOS)
        offset = (size - draw_size) // 2
        canvas.paste(resized_logo, (offset, offset), resized_logo)
        
        legacy_path = os.path.join(out_dir, "ic_launcher.png")
        round_path = os.path.join(out_dir, "ic_launcher_round.png")
        canvas.save(legacy_path)
        canvas.save(round_path)
        print(f"Generated legacy icons in {density}")

if __name__ == "__main__":
    generate_android_icons()
