package com.example.main;

import com.example.other.ClassC;
import com.example.wildcard.*;

public class ClassB {

    private ClassC classC1;
    private com.example.another.ClassC classC2;
    private WildCardClass classWildcard;

    public ClassB(
        ClassC classC1,
        com.example.another.ClassC classC2,
        WildCardClass classWildcard
    ) {
        this.classC1 = classC1;
        this.classC2 = classC2;
        this.classWildcard = classWildcard;
    }
}
