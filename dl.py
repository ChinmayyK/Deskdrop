import urllib.request, zipfile, os, shutil
url='https://dl.google.com/android/repository/commandlinetools-win-11076708_latest.zip'
zpath=os.environ['TEMP']+'/cmdline.zip'
print('Downloading...')
urllib.request.urlretrieve(url, zpath)
sdk=os.environ['LOCALAPPDATA']+'/Android/Sdk'
dest=sdk+'/cmdline-tools/latest'
os.makedirs(sdk, exist_ok=True)
print('Extracting...')
ext_path = os.environ['TEMP']+'/ext'
if os.path.exists(ext_path):
    shutil.rmtree(ext_path)
with zipfile.ZipFile(zpath, 'r') as z:
    z.extractall(ext_path)
if os.path.exists(dest):
    shutil.rmtree(dest)
os.makedirs(dest, exist_ok=True)
print('Moving...')
shutil.move(ext_path+'/cmdline-tools/bin', dest)
shutil.move(ext_path+'/cmdline-tools/lib', dest)
shutil.move(ext_path+'/cmdline-tools/source.properties', dest)
print('Download and extract complete!')
