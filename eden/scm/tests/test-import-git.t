#require py2
  >>> _ = open('binary.diff', 'wb').write(data.replace(b'\n', b'\r\n'))