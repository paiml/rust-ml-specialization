(.structured_output // empty) | select(
  (keys | sort) == ["findings","verdict"] and (.verdict | IN("PASS","FAIL")) and
  (.findings | type == "array" and all(.[]; (keys | sort) == ["class","line","why"] and
    (.line | type == "number" and floor == . and . >= 1) and (.class | type == "string" and length > 0) and
    (.why | type == "string" and length > 0))))
