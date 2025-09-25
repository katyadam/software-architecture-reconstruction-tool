I want that each **Codebase** has some existing **Configuration** eg. N:1 relationship.

**Configurations** can be assigned to multiple different **Codebases**, they have to share the same **Project**

Also each **Configuration** has to belong to some **Project**.

If **Codebase** does not have its **Configuration**, either **Codebase** gets assigned existing **Configuration** that has same project_uuid or new **Configuration** for particular **Project** is created and assigned to the **Codebase**.
