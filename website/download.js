async function main() {
    let msgElement = document.getElementById("msg")
    let params = new URLSearchParams(window.location.search.substr(1))
    let kind = params.get("kind")
    let link = params.get("link")

    try {
        msgElement.innerText = "Fetching..."
        const res = await fetch(`/${kind}?link=${encodeURIComponent(link)}`)
        if (!res.ok) {
            msgElement.innerText = `Error: ${await res.text()}`
            msgElement.className = "error"
            return
        }

        msgElement.innerText = "Downloading..."
        let temp = document.createElement("a")
        temp.href = URL.createObjectURL(await res.blob())
        let filename = res.headers.get("Content-Disposition")
            .match(/attachment; filename="(.*)"/)[1]
        if (filename == undefined) throw "no filename"
        temp.download = filename
        console.log(temp.download)
        temp.click()
        temp.remove()
    } catch (error) {
        msgElement.innerText = "Something went wrong, try again later"
        msgElement.className = "error"
        throw error
    }
}

main()
