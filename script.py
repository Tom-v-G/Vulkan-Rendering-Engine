from PIL import Image

if __name__ == "__main__":
    with Image.open('resources/frog.png') as im:
        new = im.convert('RGBA').resize((1024, 1024))
        new.save('frog2.png')
