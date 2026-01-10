import { useEffect, useState, useMemo, useRef, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';

import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { H1, H3 } from '@/components/ui/typography';
import { Input } from '@/components/ui/input';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Project } from 'shared/types';
import { showProjectForm } from '@/lib/modals';
import { projectsApi, tasksApi } from '@/lib/api';
import {
  AlertCircle,
  Loader2,
  Plus,
  ArrowDownWideNarrow,
  ArrowUpNarrowWide,
  Search,
} from 'lucide-react';
import ProjectCard from '@/components/projects/ProjectCard.tsx';
import { useKeyCreate, Scope } from '@/keyboard';

type SortField = 'activity' | 'created' | 'name';
type SortDirection = 'asc' | 'desc';

const STORAGE_KEY_FIELD = 'projectList.sortField';
const STORAGE_KEY_DIR = 'projectList.sortDirection';

export function ProjectList() {
  const navigate = useNavigate();
  const { t } = useTranslation('projects');
  const [projects, setProjects] = useState<Project[]>([]);
  const [projectActivity, setProjectActivity] = useState<Map<string, Date>>(
    new Map()
  );
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [focusedProjectId, setFocusedProjectId] = useState<string | null>(null);
  const [sortField, setSortField] = useState<SortField>(() => {
    const saved = localStorage.getItem(STORAGE_KEY_FIELD);
    return (saved as SortField) || 'activity';
  });
  const [sortDirection, setSortDirection] = useState<SortDirection>(() => {
    const saved = localStorage.getItem(STORAGE_KEY_DIR);
    return (saved as SortDirection) || 'desc';
  });
  const [searchQuery, setSearchQuery] = useState('');
  const searchInputRef = useRef<HTMLInputElement>(null);

  const fetchProjects = async () => {
    setLoading(true);
    setError('');

    try {
      const result = await projectsApi.getAll();
      setProjects(result);

      // Fetch most recent task for each project to determine real activity
      const activityMap = new Map<string, Date>();
      await Promise.all(
        result.map(async (project) => {
          try {
            const tasks = await tasksApi.getAll(project.id);
            if (tasks.length > 0) {
              // Find most recent task activity (updated_at)
              const mostRecent = tasks.reduce((latest, task) => {
                const taskDate = new Date(task.updated_at);
                return taskDate > latest ? taskDate : latest;
              }, new Date(0));
              activityMap.set(project.id, mostRecent);
            } else {
              // No tasks, use project creation date
              activityMap.set(project.id, new Date(project.created_at));
            }
          } catch {
            // If task fetch fails, fall back to project creation date
            activityMap.set(project.id, new Date(project.created_at));
          }
        })
      );
      setProjectActivity(activityMap);
    } catch (error) {
      console.error('Failed to fetch projects:', error);
      setError(t('errors.fetchFailed'));
    } finally {
      setLoading(false);
    }
  };

  const handleCreateProject = async () => {
    try {
      const result = await showProjectForm();
      if (result === 'saved') {
        fetchProjects();
      }
    } catch (error) {
      // User cancelled - do nothing
    }
  };

  const handleSortFieldChange = (value: SortField) => {
    setSortField(value);
    localStorage.setItem(STORAGE_KEY_FIELD, value);
  };

  const toggleSortDirection = () => {
    const newDirection = sortDirection === 'asc' ? 'desc' : 'asc';
    setSortDirection(newDirection);
    localStorage.setItem(STORAGE_KEY_DIR, newDirection);
  };

  const handleSearchChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    setSearchQuery(e.target.value);
    // Restore focus after state update
    requestAnimationFrame(() => {
      if (document.activeElement !== searchInputRef.current) {
        searchInputRef.current?.focus();
      }
    });
  }, []);

  // Memoize sorted and filtered projects to avoid recomputing on every render
  const sortedProjects = useMemo(() => {
    let sorted = [...projects];

    // First filter by search query
    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase();
      sorted = sorted.filter((project) =>
        project.name.toLowerCase().includes(query)
      );
    }

    // Then sort by field
    switch (sortField) {
      case 'activity':
        sorted.sort((a, b) => {
          const dateA = projectActivity.get(a.id) || new Date(a.created_at);
          const dateB = projectActivity.get(b.id) || new Date(b.created_at);
          return dateB.getTime() - dateA.getTime();
        });
        break;
      case 'created':
        sorted.sort(
          (a, b) =>
            new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
        );
        break;
      case 'name':
        sorted.sort((a, b) => a.name.localeCompare(b.name));
        break;
    }

    // Then apply direction
    if (sortDirection === 'asc') {
      sorted.reverse();
    }

    return sorted;
  }, [projects, projectActivity, sortField, sortDirection, searchQuery]);

  // Semantic keyboard shortcut for creating new project
  useKeyCreate(handleCreateProject, { scope: Scope.PROJECTS });

  const handleEditProject = (project: Project) => {
    navigate(`/settings/projects?projectId=${project.id}`);
  };

  // Set initial focus when projects are loaded
  useEffect(() => {
    if (projects.length > 0 && !focusedProjectId) {
      setFocusedProjectId(projects[0].id);
    }
  }, [projects.length, focusedProjectId]);

  useEffect(() => {
    fetchProjects();
    // eslint-disable-next-line react-hooks/exhaustive-deps -- only run on mount
  }, []);

  return (
    <div className="space-y-6 p-8 pb-16 md:pb-8 h-full overflow-auto">
      <div className="flex flex-col gap-4 sm:flex-row sm:justify-between sm:items-center">
        <div>
          <H1 className="tracking-tight">{t('title')}</H1>
          <p className="text-muted-foreground">{t('subtitle')}</p>
        </div>
        <div className="flex flex-wrap items-center gap-3">
          <div key="search-input" className="relative w-[200px]">
            <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
            <Input
              ref={searchInputRef}
              type="text"
              placeholder={t('search.placeholder')}
              value={searchQuery}
              onChange={handleSearchChange}
              className="pl-8"
            />
          </div>
          <Select value={sortField} onValueChange={handleSortFieldChange}>
            <SelectTrigger className="w-[180px]">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="activity">{t('sort.activity')}</SelectItem>
              <SelectItem value="created">{t('sort.created')}</SelectItem>
              <SelectItem value="name">{t('sort.name')}</SelectItem>
            </SelectContent>
          </Select>
          <Button
            variant="outline"
            size="icon"
            onClick={toggleSortDirection}
            title={
              sortDirection === 'desc'
                ? t('sort.descending')
                : t('sort.ascending')
            }
          >
            {sortDirection === 'desc' ? (
              <ArrowDownWideNarrow className="h-4 w-4" />
            ) : (
              <ArrowUpNarrowWide className="h-4 w-4" />
            )}
          </Button>
          <Button onClick={handleCreateProject}>
            <Plus className="mr-2 h-4 w-4" />
            {t('createProject')}
          </Button>
        </div>
      </div>

      {error && (
        <Alert variant="destructive">
          <AlertCircle className="h-4 w-4" />
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}

      {loading ? (
        <div className="flex items-center justify-center py-12">
          <Loader2 className="mr-2 h-4 w-4 animate-spin" />
          {t('loading')}
        </div>
      ) : projects.length === 0 ? (
        <Card>
          <CardContent className="py-12 text-center">
            <div className="mx-auto flex h-12 w-12 items-center justify-center rounded-lg bg-muted">
              <Plus className="h-6 w-6" />
            </div>
            <H3 className="mt-4">{t('empty.title')}</H3>
            <p className="mt-2 text-sm text-muted-foreground">
              {t('empty.description')}
            </p>
            <Button className="mt-4" onClick={handleCreateProject}>
              <Plus className="mr-2 h-4 w-4" />
              {t('empty.createFirst')}
            </Button>
          </CardContent>
        </Card>
      ) : sortedProjects.length === 0 ? (
        <Card>
          <CardContent className="py-12 text-center">
            <div className="mx-auto flex h-12 w-12 items-center justify-center rounded-lg bg-muted">
              <Search className="h-6 w-6" />
            </div>
            <H3 className="mt-4">{t('search.noResults')}</H3>
            <p className="mt-2 text-sm text-muted-foreground">
              {t('search.noResultsDescription')}
            </p>
          </CardContent>
        </Card>
      ) : (
        <div className="grid gap-6 md:grid-cols-2 lg:grid-cols-3">
          {sortedProjects.map((project) => (
            <ProjectCard
              key={project.id}
              project={project}
              isFocused={focusedProjectId === project.id}
              setError={setError}
              onEdit={handleEditProject}
              fetchProjects={fetchProjects}
            />
          ))}
        </div>
      )}
    </div>
  );
}
