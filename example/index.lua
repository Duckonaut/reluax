return function()
    return {
        tag = "html",
        children = {
            {
                tag = "head",
                children = {
                    {
                        tag = "title",
                        children = {
                            "Hello World"
                        }
                    }
                }
            },
            {
                tag = "body",
                children = {
                    {
                        tag = "h1",
                        children = {
                            "Hello World"
                        }
                    }
                }
            }
        }
    }
end
