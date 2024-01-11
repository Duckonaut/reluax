local index = require("index")

math.randomseed(os.time())

local function route(path)
    if path == '/' then
        return index()
    elseif path == '/goodbye' then
        return "goodbye"
    elseif path == '/random' then
        return 'no more random'
    end

    return nil
end

return {
    name = "hello-world",
    route = route
}
