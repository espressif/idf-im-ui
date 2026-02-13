import urllib.request

url = 'https://official-joke-api.appspot.com/random_joke'
response = urllib.request.urlopen(url)

if response.getcode() == 200:
    print("Request successful!")
    print("Response content:", response.read())
    exit(1) # this will be reverted back before merging to master
else:
    print("Request failed. Status code:", response.getcode())
    exit(1)