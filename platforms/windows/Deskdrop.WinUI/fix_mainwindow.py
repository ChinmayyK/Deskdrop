with open('/Users/chinmayk/Projects/Deskdrop/platforms/windows/Deskdrop.Windows/MainWindow.xaml', 'r') as f:
    content = f.read()

# 1. Update xmlns to include local2
if 'xmlns:local2' not in content:
    content = content.replace('xmlns:local="clr-namespace:Deskdrop.Windows"', 
                              'xmlns:local="clr-namespace:Deskdrop.Windows"\n        xmlns:local2="clr-namespace:Deskdrop.Windows.Views"')

# 2. Extract Window.Resources to AppStyles.xaml and replace
res_start = content.find('<Window.Resources>')
res_end = content.find('</Window.Resources>') + len('</Window.Resources>')

if res_start != -1 and res_end > res_start:
    new_res = '    <Window.Resources>\n        <ResourceDictionary Source="AppStyles.xaml" />\n    </Window.Resources>'
    content = content[:res_start] + new_res + content[res_end:]

# 3. Replace the three grids
def replace_grid(name):
    global content
    start_tag = f'<Grid x:Name="{name}"'
    start_idx = content.find(start_tag)
    if start_idx == -1: return
    
    count = 0
    i = start_idx
    end_idx = -1
    while i < len(content):
        if content[i:i+5] == '<Grid':
            count += 1
            i += 5
        elif content[i:i+7] == '</Grid>':
            count -= 1
            i += 7
            if count == 0:
                end_idx = i
                break
        else:
            i += 1
            
    if end_idx != -1:
        replacement = f'<local2:{name} x:Name="{name}" Visibility="Collapsed" Margin="0,16,0,0"/>'
        if name == 'ActivityView':
            replacement = f'<local2:{name} x:Name="{name}" Visibility="Visible" Margin="0,16,0,0"/>'
        content = content[:start_idx] + replacement + content[end_idx:]

replace_grid('ActivityView')
replace_grid('TransfersView')
replace_grid('DevicesView')

with open('/Users/chinmayk/Projects/Deskdrop/platforms/windows/Deskdrop.Windows/MainWindow.xaml', 'w') as f:
    f.write(content)

print("MainWindow.xaml updated successfully.")
