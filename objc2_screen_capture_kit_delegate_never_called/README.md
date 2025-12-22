This example only works on macOS due to `objc` is macOS specified.

When developping screen capture crate for all platforms, problem is faced when it comes to macOS' `objc2`.

The `Delegate`, whose counterpart is `trait` in Rust, fails to be called in an `Block`, whose counterpart is `closure` in Rust.

It is said by the owner of `objc2` repo that asynchronous `Block` is yield and never back again. 

Since the incorrect attempt has all logic in a Block, it's yield and never back again 
without running the runloop. 

The fix is to simplify the Block, and get logic out of Block, so things just work.
