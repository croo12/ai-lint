use crate::{analyzer::Analyzer, rule::RuleViolation, rules::select};

fn check_file(path: &str, source: &str) -> Vec<RuleViolation> {
    let engine = select(&["no-query-hook-mocking".into()]).unwrap();
    let result = Analyzer::analyze_source_with_rules(path, source, &engine).unwrap();
    assert!(
        result.syntax_errors.is_empty(),
        "{source}: {:?}",
        result.syntax_errors
    );
    result.rule_violations
}
fn check(source: &str) -> Vec<RuleViolation> {
    check_file("example.test.tsx", source)
}

#[test]
fn only_requested_test_extensions_are_checked() {
    let source = "vi.mock('@tanstack/react-query', () => ({ useQuery: vi.fn() }));";
    for path in [
        "sample.test.ts",
        "ui/sample.test.tsx",
        "/repo/ui/sample.browser.test.tsx",
    ] {
        assert_eq!(check_file(path, source).len(), 1, "{path}");
    }
    for path in [
        "sample.ts",
        "sample.tsx",
        "sample.spec.ts",
        "sample.test.js",
        "sample.test.jsx",
        "sample.test.ts/helper.ts",
        "test.tsx",
    ] {
        assert!(check_file(path, source).is_empty(), "{path}");
    }
}

#[test]
fn module_mocks_cover_vitest_jest_and_real_gebra_factory_forms() {
    for source in [
        "vi.mock('@tanstack/react-query');",
        "jest.mock('react-query');",
        "vi.mock('@tanstack/react-query', { spy: true });",
        "vi.doMock('@tanstack/react-query', () => ({ useQuery: vi.fn() }));",
        "jest.unstable_mockModule('@apollo/client', () => ({ useLazyQuery: jest.fn() }));",
        "vi.mock(import('@tanstack/react-query'), () => ({ useMutation: vi.fn() }));",
        "const useSuspenseQueryMock = vi.fn(); vi.mock('@tanstack/react-query', () => ({ useSuspenseQuery: useSuspenseQueryMock }));",
        "vi.mock('@tanstack/react-query', async importOriginal => { const actual = await importOriginal(); return { ...actual, useSuspenseQuery: () => ({ data: {} }) }; });",
        "vi.mock('./api/use-upload-log', () => ({ useUploadLog: vi.fn() }));",
        "vi.mock('../api', () => ({ useLatestNewFeaturePost: vi.fn() }));",
        "vi.mock('./api/use-upload-log');",
        "vi.mock('./hooks/use-users-query');",
        "vi.mock('./api/use-upload-log', () => ({ default: vi.fn() }));",
        "vi.mock('@entities/versions', () => ({ useSuspenseVersionsByIds: (...args) => mockVersions(...args) }));",
        "vi.mock('./hooks', () => ({ 'useUsersQuery': stub }));",
        "vi.mock('./hooks', () => ({ ['useUsersQuery']: stub }));",
        "vi.mock('swr', () => ({ default: vi.fn() }));",
        "vi.mock('./api/use-upload-log', function () { if (flag) return { useUploadLog: vi.fn() }; return {}; });",
    ] {
        assert_eq!(check(source).len(), 1, "{source}");
    }
}

#[test]
fn custom_hook_mocks_are_identified_by_query_or_mutation_results() {
    for source in [
        "vi.mock('@entities/project', () => ({ useProjects: vi.fn(() => ({ data: [], isLoading: false })) }));",
        "vi.mock('@features/sanger-report', () => ({ useReportHistory: () => ({ data: [], isError: false }) }));",
        "vi.mock('@features/sanger-report', () => ({ useSubmitSangerReport: () => ({ mutate: mockMutate }) }));",
        "vi.mock('@hooks/useBasicStats', () => ({ default: vi.fn(() => ({ data: undefined, error: null })) }));",
        "vi.mock('@features/qc', () => ({ useCreateQc: vi.fn().mockReturnValue({ mutate: fn }) }));",
        "vi.mocked(useBilling).mockReturnValue({ data: {}, isError: false } as ReturnType<typeof useBilling>);",
        "import { useProjects } from '@entities/project'; const mockUseProjects = vi.mocked(useProjects); mockUseProjects.mockReturnValue({ data: [], isLoading: false });",
        "vi.spyOn(hooks, 'useSubmitReport').mockReturnValue({ mutate: fn });",
        "vi.mock('./hooks', () => ({ useProjects: vi.fn(function () { return { data: [], isLoading: false }; }) }));",
    ] {
        assert_eq!(check(source).len(), 1, "{source}");
    }
}

#[test]
fn imported_aliases_spies_and_typed_mock_configuration_are_checked() {
    for source in [
        "import { vi as testMock } from 'vitest'; testMock.mock('@tanstack/react-query');",
        "import { jest as testMock } from '@jest/globals'; testMock.spyOn(hooks, 'useQuery');",
        "import { useQuery as query } from '@tanstack/react-query'; vi.mocked(query).mockReturnValue({});",
        "import { useQuery as query } from '@tanstack/react-query'; const mockedQuery = vi.mocked(query); mockedQuery.mockImplementation(() => ({}));",
        "import * as hooks from './api/use-reanalysis-upload'; vi.spyOn(hooks, 'useReanalysisUpload').mockReturnValue({ mutate: fn });",
        "jest.spyOn(hooks, 'useInfiniteQuery').mockImplementation(() => ({}));",
        "jest.replaceProperty(hooks, 'useQuery', jest.fn());",
        "(useQuery as jest.Mock).mockReturnValue({});",
        "vi.mocked(hooks['useQuery']).mockReturnValueOnce({});",
        "hooks.useMutation.mockImplementationOnce(() => ({}));",
        "import read from 'swr'; vi.mocked(read).mockReturnValue({});",
        "import read from './hooks/use-users-query'; vi.mocked(read).mockReturnValue({});",
        "import { useUploadLog as upload } from '../api'; vi.mocked(upload).mockReturnValue({});",
        "import { useUsersQuery } from './queries'; vi.mock('./queries');",
        "useQuery.mockReturnValue({}).mockReturnValueOnce({});",
    ] {
        assert_eq!(check(source).len(), 1, "{source}");
    }
}

#[test]
fn msw_ui_auth_and_non_mock_uses_remain_allowed() {
    for source in [
        "server.use(http.get('/projects', () => HttpResponse.json([])));",
        "server.use(graphql.query('Projects', () => HttpResponse.json({ data: { projects: [] } })));",
        "const result = useQuery({ queryKey: ['projects'], queryFn: fetchProjects });",
        "vi.mocked(useQuery); expect(vi.mocked(useQuery)).toHaveBeenCalled();",
        "vi.mocked(useQuery).mockClear(); vi.mocked(useQuery).mockReset();",
        "vi.mock('react-router-dom', () => ({ useNavigate: vi.fn(), useParams: () => ({ id: '1' }) }));",
        "vi.mock('@entities/auth', () => ({ useUser: vi.fn(() => ({ isInternalUser: true })) }));",
        "vi.mocked(useUser).mockReturnValue({ currentUser: 'tester' });",
        "vi.spyOn(auth, 'useUser').mockReturnValue({ isSaasUser: true });",
        "vi.spyOn(console, 'error').mockImplementation(() => {});",
        "vi.mock('@features/state', () => ({ useLocalState: () => ({ data: [], selected: false }) }));",
        "vi.mock('./state', () => ({ useSelectedVersionsStore: selector => mockStore(selector) }));",
        "vi.mock('./ui', () => ({ Component: () => ({ data: [], isLoading: false }) }));",
        "vi.mock('@tanstack/react-query', async importOriginal => ({ ...(await importOriginal()), QueryClient: vi.fn() }));",
        "vi.mock('./api', () => ({ fetchProjects: vi.fn() }));",
        "vi.mock('@entities/project', () => ({ useProjects: vi.fn(() => { const unrelated = { data: [], isLoading: false }; return []; }) }));",
        "// vi.mock('@tanstack/react-query')\nconst text = 'useQuery.mockReturnValue({})';",
    ] {
        assert!(check(source).is_empty(), "{source}");
    }
}

#[test]
fn preserved_actual_hooks_and_shadowed_bindings_are_not_mocks() {
    for source in [
        "vi.mock('@tanstack/react-query', async importOriginal => { const actual = await importOriginal(); return { ...actual, useQuery: actual.useQuery }; });",
        "vi.mock('@tanstack/react-query', async load => { const actual = await load(); return { useQuery: actual.useQuery }; });",
        "import { useQuery } from '@tanstack/react-query'; vi.mock('@tanstack/react-query', () => ({ useQuery }));",
        "vi.mock('./hooks', () => { const actual = jest.requireActual('./hooks'); return { useUsersQuery: actual.useUsersQuery }; });",
        "import { useQuery as query } from '@tanstack/react-query'; function f(query) { vi.mocked(query).mockReturnValue({}); }",
        "import { vi as mocks } from 'vitest'; function f(mocks) { mocks.mock('@tanstack/react-query'); }",
        "function f(vi) { vi.mock('@tanstack/react-query'); }",
        "const vi = customMocker; vi.mock('@tanstack/react-query');",
        "import { useNavigate as useQuery } from 'react-router-dom'; vi.mocked(useQuery).mockReturnValue({});",
    ] {
        assert!(check(source).is_empty(), "{source}");
    }
}

#[test]
fn a_function_named_import_original_is_not_evidence_of_a_real_hook() {
    for source in [
        "vi.mock('@tanstack/react-query', () => { const importOriginal = vi.fn(); const actual = importOriginal(); return { useQuery: actual.useQuery }; });",
        "vi.mock('@tanstack/react-query', () => { const actual = jest.requireActual('./fake-hooks'); return { useQuery: actual.useQuery }; });",
    ] {
        assert_eq!(check(source).len(), 1, "{source}");
    }
}

#[test]
fn diagnostics_point_to_the_mock_and_need_no_model() {
    let mock =
        "vi.mock('@tanstack/react-query', () => ({ useQuery: vi.fn(), useMutation: vi.fn() }))";
    let source = format!("// 한글\n{mock};");
    let found = check(&source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].rule_id, "no-query-hook-mocking");
    assert!(found[0].message.contains("MSW"));
    assert_eq!(
        &source[found[0].span.start as usize..found[0].span.end as usize],
        mock
    );
}

#[test]
fn alias_cycles_and_nested_function_returns_terminate_without_false_positives() {
    for source in [
        "const a = b; const b = a; a.mockReturnValue({});",
        "const a = b; const b = a; vi.mock('./hooks', () => ({ useProjects: a }));",
        "vi.mock('./hooks', () => ({ useProjects: () => { function nested() { return { data: [], isLoading: false }; } return []; } }));",
    ] {
        assert!(check(source).is_empty(), "{source}");
    }
}
