This example only works on macOS due to `objc` is macOS specified.

When developping screen capture crate for all platforms, problem is faced when it comes to macOS.

The frames captured on M1 MacBook are correct, however, when testing on M3 MacBook, 
the pixels in the frame got messed up: 
some pixels at the beginning of each line will move up to the end of the line above.

The reason is `CVPixelBufferGetWidth` does not return correct width as expected:
it returns results without padding as is said in [Technical Q&A QA1829 Understanding the bytes per row value returned by CVPixelBufferGetBytesPerRow](https://developer.apple.com/library/archive/qa/qa1829/_index.html).

The fix is `CVPixelBufferGetBytesPerRow(&buffer) / 4` if pixel size is 4 bytes.
