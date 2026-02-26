import urllib.request

url = 'https://dl.espressif.com/'
response = urllib.request.urlopen(url)

if response.getcode() == 200:
    print("Request successful!")
    print("Response content:", response.read())
else:
    print("Request failed. Status code:", response.getcode())
    exit(1)
