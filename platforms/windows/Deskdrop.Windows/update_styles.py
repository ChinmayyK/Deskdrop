import os
import re

def update_xaml(file_path):
    with open(file_path, 'r', encoding='utf-8') as f:
        content = f.read()

    # 1. Update Colors to Fluent Design
    # Change Mac Brand Electric (Blue) to Fluent Blue
    content = content.replace('<Color x:Key="BrandElectricColor">#007AFF</Color>', '<Color x:Key="BrandElectricColor">#005FB8</Color>')
    
    # Update Surface colors to be semi-transparent for Mica to shine through
    # For Light mode with Mica, usually background is transparent and borders are subtle.
    # Let's use a subtle fluent palette
    content = content.replace('<SolidColorBrush x:Key="SurfaceElevated" Color="#FFFFFF"/>', '<SolidColorBrush x:Key="SurfaceElevated" Color="#F3FFFFFF"/>')
    content = content.replace('<SolidColorBrush x:Key="SurfaceStrong" Color="#F2F2F7"/>', '<SolidColorBrush x:Key="SurfaceStrong" Color="#A0F3F3F3"/>')
    content = content.replace('<SolidColorBrush x:Key="SurfaceGlass" Color="#B3FFFFFF"/>', '<SolidColorBrush x:Key="SurfaceGlass" Color="#80FFFFFF"/>')
    
    # 2. Update Window Styles and Corners to Fluent (usually 8px or 4px)
    content = content.replace('CornerRadius="12"', 'CornerRadius="8"')
    content = content.replace('CornerRadius="16"', 'CornerRadius="8"')
    content = content.replace('CornerRadius="24"', 'CornerRadius="8"')
    
    # 3. Rename Mac-specific terms
    content = content.replace('macOS CRTheme Palette', 'Windows 11 Fluent Palette')
    content = content.replace('macOS Traffic Light', 'Fluent')
    content = content.replace('macOS Deskdrop Floating Navbar', 'Fluent Navigation Bar')
    content = content.replace('macOS Style Modern Button', 'Fluent Style Button')
    content = content.replace('macOS Style Search/TextBox', 'Fluent Search/TextBox')
    
    # 4. Modify CapsuleNavButtonStyle to look more like Windows Toggle Buttons
    # Remove the heavy drop shadow from active nav button, use a subtle accent underline or background
    content = re.sub(
        r'<DropShadowEffect Color="#007AFF" Opacity="0.35" BlurRadius="8" ShadowDepth="3"/>',
        '<DropShadowEffect Color="#005FB8" Opacity="0.2" BlurRadius="4" ShadowDepth="1"/>',
        content
    )
    
    # 5. Fix Focus halo to be standard 2px border bottom (Windows style) instead of Mac blue halo
    # From: <Setter TargetName="border" Property="BorderThickness" Value="3"/>
    content = content.replace('<Setter TargetName="border" Property="BorderThickness" Value="3"/>', 
                              '<Setter TargetName="border" Property="BorderThickness" Value="2"/>')
                              
    # 6. Change top chrome title bar to look native
    # Current top chrome: <Border.Background><SolidColorBrush Color="#FFFFFF" Opacity="0.95"/></Border.Background>
    # Make it fully transparent so Mica shows
    content = content.replace('<SolidColorBrush Color="#FFFFFF" Opacity="0.95"/>', '<SolidColorBrush Color="Transparent" />')
    
    # 7. Bottom Chrome navbar: change from glass to a more Fluent floating bar
    content = content.replace('CornerRadius="24" Background="{StaticResource SurfaceGlass}"', 'CornerRadius="8" Background="{StaticResource SurfaceGlass}"')
    
    with open(file_path, 'w', encoding='utf-8') as f:
        f.write(content)

def main():
    base_dir = '/Users/chinmayk/Projects/Deskdrop/platforms/windows/Deskdrop.Windows'
    for f in os.listdir(base_dir):
        if f.endswith('.xaml'):
            update_xaml(os.path.join(base_dir, f))

if __name__ == '__main__':
    main()
