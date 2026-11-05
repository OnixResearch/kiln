package main

import (
	"fmt"
	"os"
)

func main() {
	if len(os.Args) != 2 || os.Args[0] != "/test" || (os.Args[1] != "pass" && os.Args[1] != "fail") {
		panic("unexpected VMM or loader argument forwarded")
	}
	fmt.Printf("static Go native test argv=%q\n", os.Args)
	if os.Args[1] == "fail" {
		os.Exit(7)
	}
}
