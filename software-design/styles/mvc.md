# Model-View-Controller (MVC)

## Use when
- A web or GUI application built on a framework that is organized around MVC (Rails, Django, Laravel, ASP.NET MVC, Spring MVC, many desktop toolkits).
- Following the framework's conventions is worth more than a custom architecture.

## Structure
- **Model**: data and the business rules about it. In larger applications, rules that span several models go to service objects next to the models.
- **View**: rendering only (templates, components, widgets). No queries, no rules, no side effects.
- **Controller**: receives the request or UI event, calls models or services, chooses the view. Controllers stay thin.

## Dependency rule
Views read what controllers give them; controllers call models and services; models know nothing about controllers or views. Framework conventions for folders and naming are followed exactly, so newcomers find things where the framework says they are.

## Keeping it maintainable
- **Thin controllers**: more than parameter handling, one service call and a response is a sign that logic belongs in a model or service.
- **Service objects** for operations that touch several models, call external services or run in the background.
- **No logic in templates** beyond presentation (formatting, loops, conditionals on display state).
- **Queries in models or query objects**, never in views.
- **Background jobs** call the same services as controllers.

## Where things go
| Piece | Location |
|---|---|
| Validation of a field's business meaning | Model |
| Operation across several models or an external API | Service object |
| Parsing request parameters, authorization check, redirect | Controller |
| HTML, component layout, formatting | View |

## Common mistakes
- "Fat controllers" with business rules, queries and formatting together.
- Callbacks on models that send email or call APIs, hiding side effects.
- Views running queries (N+1 problems and untestable rules).
- Bypassing the framework's structure with ad-hoc folders that only one person understands.
