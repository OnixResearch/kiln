(import "../web-cache.ncl") ((import "web-cache-positive.ncl") & { projects | force = [(import "web-cache-positive.ncl").projects [0] & { paths | force = ["target/cache", "target/cache"] }] })
